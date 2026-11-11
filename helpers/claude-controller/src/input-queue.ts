/** Keeps SDK stdin open while Carlos waits for the next user message. */
export class InputQueue<T> implements AsyncIterable<T> {
  private values: T[] = [];
  private waiter?: (result: IteratorResult<T>) => void;
  private ended = false;

  push(value: T): void {
    if (this.ended) throw new Error("The SDK input stream is closed");
    if (!this.waiter) { this.values.push(value); return; }
    const resolve = this.waiter;
    this.waiter = undefined;
    resolve({ value, done: false });
  }

  close(): void {
    this.ended = true;
    this.values = [];
    this.waiter?.({ value: undefined, done: true });
    this.waiter = undefined;
  }

  [Symbol.asyncIterator](): AsyncIterator<T> {
    return { next: () => {
      const value = this.values.shift();
      if (value !== undefined) return Promise.resolve({ value, done: false });
      if (this.ended) return Promise.resolve({ value: undefined, done: true });
      return new Promise((resolve) => { this.waiter = resolve; });
    } };
  }
}
