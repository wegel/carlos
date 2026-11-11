/** Validates native question answers before the SDK settles a callback. */
import { matchesFormat } from "./formats";
import { isDeepStrictEqual } from "node:util";
import { object, ProtocolError, type ObjectValue } from "./protocol";

const annotations = ["title", "description", "default"];

/** Check callback-specific inputs before the helper returns a native answer. */
export function validateQuestionAnswer(kind: string, payload: ObjectValue, answer: ObjectValue): void {
  if (kind === "permission" && answer.behavior === "allow" && payload.toolName === "AskUserQuestion") {
    const input = object(payload.input, "question input");
    const updated = object(answer.updatedInput, "answer.updatedInput");
    if (!isDeepStrictEqual(updated.questions, input.questions)) fail("Questions must remain unchanged");
    const answers = object(updated.answers, "answer.updatedInput.answers");
    if (!Array.isArray(input.questions) || input.questions.length === 0) fail("No questions were supplied");
    for (const question of input.questions) {
      const text = object(question, "question").question;
      if (typeof text !== "string" || !Object.hasOwn(answers, text) || typeof answers[text] !== "string" || !answers[text].trim()) {
        fail(`Answer the question: ${String(text)}`);
      }
    }
  }
  if (kind !== "elicitation" || answer.action !== "accept") return;
  if (payload.mode !== undefined && payload.mode !== "url" && payload.mode !== "form") fail("Unsupported question mode");
  if (payload.mode === "url") {
    if (typeof payload.url !== "string" || !/^https?:\/\//iu.test(payload.url) || !matchesFormat(payload.url, "uri")) fail("Browser question requires a valid HTTP(S) URL");
    try { new URL(payload.url); } catch { fail("Browser question has an invalid URL"); }
    if (answer.content !== undefined) fail("Browser consent must not contain form answers");
    return;
  }
  validateForm(payload.requestedSchema, answer.content);
}

/** Check typed values against the supported flat MCP form rules. */
export function validateForm(value: unknown, content: unknown): void {
  const { properties, required } = validateFormSchema(value);
  const answers = object(content, "answer.content");
  for (const key of Object.keys(answers)) if (!Object.hasOwn(properties, key)) fail(`Unknown field: ${key}`);
  for (const [name, field] of Object.entries(properties)) {
    const rule = object(field, name);
    if (!Object.hasOwn(answers, name)) {
      if (required.includes(name)) fail(`${name}: a value is required`);
      continue;
    }
    validateValue(rule, answers[name], name);
  }
}

/** Describe unsupported form shapes without running server patterns on the dispatcher. */
export function formIssue(value: unknown): string | undefined {
  try { validateFormSchema(value); } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
}

function validateFormSchema(value: unknown): { properties: ObjectValue; required: string[] } {
  const schema = object(value, "requestedSchema");
  keys(schema, ["type", "properties", "required", "$schema", ...annotations]);
  if (schema.type !== "object") fail("Unsupported form: expected a flat object schema");
  const properties = object(schema.properties, "requestedSchema.properties");
  const required = schema.required ?? [];
  if (!Array.isArray(required) || required.some(key => typeof key !== "string" || !Object.hasOwn(properties, key))) {
    fail("Unsupported form: required must name known fields");
  }
  if (new Set(required).size !== required.length) fail("Unsupported form: duplicate required fields");
  for (const [name, field] of Object.entries(properties)) {
    const rule = object(field, name);
    validateSchema(rule, name);
  }
  return { properties, required: required as string[] };
}

function validateSchema(rule: ObjectValue, name: string): void {
  const common = ["type", ...annotations];
  switch (rule.type) {
    case "string":
      keys(rule, [...common, "minLength", "maxLength", "pattern", "format", "enum", "enumNames", "oneOf"]);
      bounds(rule, "minLength", "maxLength", true);
      if (rule.pattern !== undefined) {
        if (typeof rule.pattern !== "string") fail(`${name}: pattern must be text`);
      }
      if (rule.format !== undefined && !["email", "uri", "date", "date-time"].includes(String(rule.format))) {
        fail(`${name}: unsupported format ${String(rule.format)}`);
      }
      if (rule.enum !== undefined || rule.oneOf !== undefined || rule.enumNames !== undefined) enumValues(rule);
      break;
    case "number": case "integer":
      keys(rule, [...common, "minimum", "maximum"]);
      bounds(rule, "minimum", "maximum", false);
      break;
    case "boolean": keys(rule, common); break;
    case "array":
      keys(rule, [...common, "items", "minItems", "maxItems"]);
      bounds(rule, "minItems", "maxItems", true);
      enumValues(object(rule.items, `${name}.items`), true);
      break;
    default: fail(`${name}: unsupported field type ${String(rule.type)}`);
  }
}

function enumValues(rule: ObjectValue, array = false): string[] {
  if (array) keys(rule, ["type", "enum", "enumNames", "anyOf"]);
  const count = ["enum", "oneOf", "anyOf"].filter(key => Object.hasOwn(rule, key)).length;
  if (count !== 1) fail("Unsupported form: use exactly one choice constraint");
  if (array && rule.enum !== undefined && rule.type !== "string") fail("Unsupported form: array items must be strings");
  if (array && rule.anyOf !== undefined && rule.type !== undefined && rule.type !== "string") fail("Unsupported form: array items must be strings");
  const options = rule.enum ?? rule.oneOf ?? rule.anyOf;
  if (!Array.isArray(options) || options.length === 0) fail("Unsupported form: empty choices");
  const values = options.map(option => {
    if (rule.enum !== undefined) {
      if (typeof option !== "string") fail("Unsupported form: enum values must be strings");
      return option;
    }
    const entry = object(option, "choice");
    keys(entry, ["const", "title"]);
    if (typeof entry.const !== "string" || typeof entry.title !== "string") fail("Unsupported titled choice");
    return entry.const;
  });
  if (new Set(values).size !== values.length) fail("Unsupported form: duplicate choices");
  if (rule.enumNames !== undefined && (!Array.isArray(rule.enumNames) || rule.enumNames.length !== values.length
    || rule.enumNames.some(name => typeof name !== "string") || rule.enum === undefined)) fail("Unsupported enumNames labels");
  return values;
}

function validateValue(rule: ObjectValue, value: unknown, name: string): void {
  switch (rule.type) {
    case "string": {
      if (typeof value !== "string") fail(`${name}: enter text`);
      compare([...value].length, rule.minLength, rule.maxLength, name, "characters");
      if (typeof rule.pattern === "string") {
        let pattern: RegExp;
        try { pattern = new RegExp(rule.pattern, "u"); } catch { fail(`${name}: unsupported pattern`); }
        if (!pattern.test(value)) fail(`${name}: must match ${rule.pattern}`);
      }
      if (typeof rule.format === "string" && !matchesFormat(value, rule.format)) fail(`${name}: enter a valid ${rule.format}`);
      if ((rule.enum !== undefined || rule.oneOf !== undefined) && !enumValues(rule).includes(value)) fail(`${name}: choose a listed value`);
      break;
    }
    case "number": case "integer":
      if (typeof value !== "number" || !Number.isFinite(value)) fail(`${name}: enter a finite ${String(rule.type)}`);
      if (rule.type === "integer" && !Number.isSafeInteger(value)) fail(`${name}: enter a safe integer between -9007199254740991 and 9007199254740991; larger integers lose precision`);
      compare(value, rule.minimum, rule.maximum, name, "value");
      break;
    case "boolean": if (typeof value !== "boolean") fail(`${name}: choose true or false`); break;
    case "array": {
      const choices = enumValues(object(rule.items, "items"), true);
      if (!Array.isArray(value) || value.some(item => typeof item !== "string" || !choices.includes(item))) fail(`${name}: choose listed values`);
      if (new Set(value).size !== value.length) fail(`${name}: remove duplicate values`);
      compare(value.length, rule.minItems, rule.maxItems, name, "selections");
      break;
    }
  }
}

function bounds(rule: ObjectValue, min: string, max: string, integral: boolean): void {
  for (const key of [min, max]) {
    const value = rule[key];
    if (value !== undefined && (typeof value !== "number" || !Number.isFinite(value) || (integral && (!Number.isInteger(value) || value < 0)))) fail(`Unsupported form: invalid ${key}`);
  }
  if (typeof rule[min] === "number" && typeof rule[max] === "number" && rule[min] > rule[max]) fail(`Unsupported form: ${min} exceeds ${max}`);
}

function compare(value: number, min: unknown, max: unknown, name: string, unit: string): void {
  if (typeof min === "number" && value < min) fail(`${name}: at least ${min} ${unit}`);
  if (typeof max === "number" && value > max) fail(`${name}: at most ${max} ${unit}`);
}

function keys(value: ObjectValue, allowed: string[]): void {
  for (const key of Object.keys(value)) if (!allowed.includes(key)) fail(`Unsupported form schema: ${key}`);
}

function fail(message: string): never { throw new ProtocolError("invalid_answer", message); }
