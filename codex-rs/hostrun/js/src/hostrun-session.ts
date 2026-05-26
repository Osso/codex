import {
  getQuickJS,
  type QuickJSContext,
  type QuickJSRuntime,
  type QuickJSWASMModule,
} from "quickjs-emscripten";

const MEMORY_LIMIT = 64 * 1024 * 1024;
const INTERRUPT_CYCLES = 100000;

export interface HostrunSessionOptions {
  interruptCycles?: number;
  memoryLimitBytes?: number;
}

export interface HostrunEvalResult {
  value: unknown;
}

export class HostrunSessionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "HostrunSessionError";
  }
}

export class HostrunSession {
  private readonly runtime: QuickJSRuntime;
  private readonly context: QuickJSContext;
  private disposed = false;

  private constructor(runtime: QuickJSRuntime, context: QuickJSContext) {
    this.runtime = runtime;
    this.context = context;
  }

  static async create(
    options: HostrunSessionOptions = {},
  ): Promise<HostrunSession> {
    const quickjs = await getQuickJS();
    return HostrunSession.createWithQuickJs(quickjs, options);
  }

  evalSync(code: string): HostrunEvalResult {
    this.assertOpen();

    const result = this.context.evalCode(code, "hostrun-session.js");
    if (result.error) {
      const error = this.context.dump(result.error);
      result.error.dispose();
      if (isFatalQuickJsError(error)) {
        this.dispose();
      }
      throw new HostrunSessionError(formatError(error));
    }

    const value = this.context.dump(result.value);
    result.value.dispose();
    this.runtime.executePendingJobs();

    return { value };
  }

  async eval(code: string): Promise<HostrunEvalResult> {
    return this.evalSync(code);
  }

  dispose(): void {
    if (this.disposed) {
      return;
    }

    this.context.dispose();
    this.runtime.dispose();
    this.disposed = true;
  }

  private static createWithQuickJs(
    quickjs: QuickJSWASMModule,
    options: HostrunSessionOptions,
  ): HostrunSession {
    const runtime = quickjs.newRuntime();
    runtime.setMemoryLimit(options.memoryLimitBytes ?? MEMORY_LIMIT);
    runtime.setInterruptHandler(
      createInterruptHandler(options.interruptCycles ?? INTERRUPT_CYCLES),
    );

    const context = runtime.newContext();
    initializeContext(context);

    return new HostrunSession(runtime, context);
  }

  private assertOpen(): void {
    if (this.disposed) {
      throw new HostrunSessionError("Hostrun session is closed");
    }
  }
}

function createInterruptHandler(interruptCycles: number): () => boolean {
  let interruptCount = 0;

  return () => {
    interruptCount++;
    return interruptCount > interruptCycles;
  };
}

function initializeContext(context: QuickJSContext): void {
  const result = context.evalCode(
    `{
      globalThis.ctx = Object.create(null);
      Object.defineProperty(globalThis, 'eval', {
        value: undefined,
        writable: false,
        configurable: false
      });
    }`,
    "hostrun-bootstrap.js",
  );

  if (result.error) {
    const error = context.dump(result.error);
    result.error.dispose();
    throw new HostrunSessionError(formatError(error));
  }

  result.value.dispose();
}

function formatError(error: unknown): string {
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }

  return String(error);
}

function isFatalQuickJsError(error: unknown): boolean {
  if (
    typeof error !== "object" ||
    error === null ||
    !("message" in error) ||
    typeof error.message !== "string"
  ) {
    return false;
  }

  return (
    error.message === "interrupted" || error.message.includes("out of memory")
  );
}
