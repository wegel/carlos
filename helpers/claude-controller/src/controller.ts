/** Owns one SDK query and correlates local requests without blocking SDK callbacks. */
import type { Query, SDKUserMessage } from "@anthropic-ai/claude-agent-sdk";

import { Callbacks } from "./callbacks";
import { InputQueue } from "./input-queue";
import { OptionalUsage } from "./optional-usage";
import { NativeOutput } from "./native-output";
import { OutputError, type OutputSink } from "./output-writer";
import { object, parseEffort, parseMode, parsePrompt, parseRequest, parseStart,
  ProtocolError, SDK_VERSION, string, type ObjectValue, type Output, type Request } from "./protocol";
import { officialQuery, startQuery, type QueryFactory } from "./sdk";
import { Sessions } from "./sessions";
import { RecentIds } from "./recent-ids";

export class Controller {
  private input = new InputQueue<SDKUserMessage>();
  private callbacks: Callbacks;
  private query?: Query;
  private state: "new" | "starting" | "ready" | "closed" = "new";
  private ids = new RecentIds();
  private promptIds = new Set<string>();
  private allowBypass = false;
  private usage = new OptionalUsage();
  private closed = new AbortController();
  private native: NativeOutput;
  private sessions = new Sessions(this.closed.signal);

  constructor(private output: OutputSink, private factory: QueryFactory = officialQuery,
    private onClosed: (reason: string) => void = () => {}) {
    this.callbacks = new Callbacks((frame) => output.enqueue(frame));
    this.native = new NativeOutput((error) => this.fail(error));
  }

  async receive(line: string): Promise<void> {
    let request: Request | undefined;
    let ownsId = false;
    let reply: Output;
    try {
      request = parseRequest(line);
      this.ids.begin(request.id);
      ownsId = true;
      const result = request.method === "shutdown" || request.method === "hello"
        ? await this.dispatch(request) : await this.untilClosed(this.dispatch(request));
      reply = { v: 1, type: "reply", id: request.id, ok: true, result: result ?? null };
    } catch (error) {
      reply = { v: 1, type: "reply", id: request?.id ?? recoverId(line), ok: false,
        error: { code: error instanceof ProtocolError ? error.code : "sdk_error", message: errorMessage(error) } };
    }
    try { await this.output.write(reply); }
    catch {} // The writer owns terminal pipe failures and closes the controller.
    finally { if (ownsId) this.ids.complete(request!.id); }
  }

  fail(error: OutputError): void {
    this.native.stop();
    if (this.state === "closed") return;
    this.output.enqueue({ v: 1, type: "event", event: "error", error: { code: error.code, message: error.message } });
    this.close(error.code);
  }

  close(reason: string): void {
    if (this.state === "closed") return;
    this.state = "closed";
    this.closed.abort();
    this.callbacks.cancelAll(reason);
    this.input.close();
    this.sessions.close();
    this.query?.close();
    this.output.enqueue({ v: 1, type: "event", event: "closed", reason });
    this.onClosed(reason);
  }

  private async dispatch({ method, params }: Request): Promise<unknown> {
    if (method === "hello") return { protocolVersion: 1, sdkVersion: SDK_VERSION,
      methods: ["hello", "start", "prompt", "callback.respond", "interrupt", "model.set", "effort.set",
        "permission.set", "context", "capabilities", "shutdown", "sessions.list", "sessions.info", "sessions.read",
        "sessions.rename", "sessions.forkBefore", "sessions.children", "sessions.metadata", "task.stop", "usage.optional"] };
    if (method === "shutdown") { this.close("shutdown"); return null; }
    if (this.state === "closed") throw new ProtocolError("closed", "The controller is closed");
    if (method.startsWith("sessions.")) return this.sessions.dispatch(method, params);
    if (method === "start") return this.start(params);
    if (method === "callback.respond") { await this.callbacks.answer(params); return null; }
    if (method === "interrupt" && this.query) return this.query.interrupt();
    const query = this.requireReady();
    switch (method) {
      case "prompt": return this.prompt(params);
      case "task.stop": {
        const taskId = string(params.taskId, "taskId");
        await query.stopTask(taskId);
        return { taskId, stopRequested: true };
      }
      case "usage.optional": return this.usage.read(query, this.closed.signal);
      case "interrupt": return query.interrupt();
      case "model.set": return query.setModel(params.model === null ? undefined : string(params.model, "model"));
      case "effort.set": return query.applyFlagSettings({ effortLevel: params.effort === null ? null : parseEffort(params.effort) });
      case "permission.set": return this.setPermission(query, params);
      case "context": return query.getContextUsage({ detail: "summary" });
      case "capabilities": return query.initializationResult();
      default: throw new ProtocolError("unknown_method", `Unknown method ${method}`);
    }
  }

  private async start(params: ObjectValue): Promise<unknown> {
    if (this.state !== "new") throw new ProtocolError("already_started", "This controller accepts one start request");
    const start = parseStart(params);
    this.state = "starting";
    this.allowBypass = start.allowDangerouslySkipPermissions === true;
    try {
      const query = startQuery(start, this.input, this.callbacks, (frame) => {
        if (this.state !== "closed") this.output.enqueue(frame);
      }, this.native, this.factory);
      this.query = query;
      void this.readEvents(query);
      const capabilities = await this.untilClosed(query.initializationResult());
      if (this.state as string === "closed") throw new ProtocolError("closed", "The CLI closed during startup");
      this.state = "ready";
      return capabilities;
    } catch (error) { this.close("start_failed"); throw error; }
  }

  private prompt(params: ObjectValue): unknown {
    const prompt = parsePrompt(params);
    if (this.promptIds.has(prompt.uuid)) throw new ProtocolError("duplicate_prompt", "Prompt UUID was already submitted");
    this.sessions.sources.remember(prompt.uuid, prompt.origin.kind);
    this.promptIds.add(prompt.uuid);
    this.input.push({ type: "user", uuid: prompt.uuid, parent_tool_use_id: null,
      message: { role: "user", content: prompt.text }, priority: prompt.priority, origin: prompt.origin });
    return { uuid: prompt.uuid, accepted: true };
  }

  private async readEvents(query: Query): Promise<void> {
    try {
      for await (const message of query) {
        this.native.delivered();
        if (this.state === "closed") break;
        await this.output.write({ v: 1, type: "event", event: "sdk", message });
      }
      if (this.state !== "closed") this.close("sdk_eof");
    } catch (error) {
      if (this.state === "closed") return;
      this.output.enqueue({ v: 1, type: "event", event: "error", error: { code: "sdk_stream_error", message: errorMessage(error) } });
      this.close("sdk_error");
    }
  }

  private requireReady(): Query {
    if (this.state !== "ready" || !this.query) throw new ProtocolError("not_ready", "Start the controller and wait for its reply first");
    return this.query;
  }

  private setPermission(query: Query, params: ObjectValue): Promise<void> {
    const mode = parseMode(params.mode);
    if (mode === "bypassPermissions" && !this.allowBypass) {
      throw new ProtocolError("invalid_request", "Bypass was not explicitly enabled at start");
    }
    return query.setPermissionMode(mode);
  }

  private untilClosed<T>(operation: Promise<T>): Promise<T> {
    const signal = this.closed.signal;
    if (signal.aborted) {
      void operation.catch(() => {});
      return Promise.reject(new ProtocolError("closed", "The controller is closed"));
    }
    return new Promise((resolve, reject) => {
      const abort = () => reject(new ProtocolError("closed", "The controller closed before the request finished"));
      signal.addEventListener("abort", abort, { once: true });
      void operation.then(resolve, reject).finally(() => signal.removeEventListener("abort", abort));
    });
  }
}

function recoverId(line: string): string | null {
  try { const value = object(JSON.parse(line), "request"); return typeof value.id === "string" ? value.id : null; }
  catch { return null; }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
