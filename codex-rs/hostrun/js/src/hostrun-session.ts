import {
  getQuickJS,
  type QuickJSContext,
  type QuickJSHandle,
  type QuickJSRuntime,
  type QuickJSWASMModule,
} from "quickjs-emscripten";

const MEMORY_LIMIT = 64 * 1024 * 1024;
const INTERRUPT_CYCLES = 100000;
const APPROVAL_REQUIRED_PREFIX = "__HOSTRUN_APPROVAL_REQUIRED__:";

export interface HostrunSessionOptions {
  approve?: HostrunApprovalHandler;
  capabilities?: Record<string, HostrunCapability>;
  interruptCycles?: number;
  memoryLimitBytes?: number;
}

export interface HostrunEvalResult {
  approval?: HostrunApprovalRequest;
  type?: "completed" | "needs_approval";
  value: unknown;
}

export interface HostrunApprovalRequest {
  id: string;
  tool: string;
  summary: string;
  args: unknown;
}

export type HostrunApprovalDecision =
  | { type: "approve" }
  | { type: "deny"; reason: string }
  | { type: "pending" };

export type HostrunApprovalHandler = (
  request: HostrunApprovalRequest,
) => HostrunApprovalDecision;

export interface HostrunCapability {
  describe: (args: Record<string, unknown>) => HostrunApprovalRequest;
  invoke: (args: Record<string, unknown>) => unknown;
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
      const pendingApproval = parsePendingApproval(error);
      if (pendingApproval) {
        return {
          type: "needs_approval",
          approval: pendingApproval,
          value: undefined,
        };
      }
      if (isFatalQuickJsError(error)) {
        this.dispose();
      }
      throw new HostrunSessionError(formatError(error));
    }

    const value = this.context.dump(result.value);
    result.value.dispose();
    this.runtime.executePendingJobs();

    return { type: "completed", value };
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
    initializeContext(context, options);

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

function initializeContext(
  context: QuickJSContext,
  options: HostrunSessionOptions,
): void {
  installToolInvoker(context, options);

  const result = context.evalCode(
    `{
      globalThis.ctx = Object.create(null);
      globalThis.tools = (function makeProxy(path) {
        return new Proxy(function() {}, {
          get: function(_target, prop) {
            if (prop === 'then' || typeof prop === 'symbol') return undefined;
            return makeProxy(path.concat([String(prop)]));
          },
          apply: function(_target, _this, args) {
            var toolPath = path.join('.');
            if (!toolPath) throw new Error('Tool path missing in invocation');
            var argsJson = args.length > 0 ? JSON.stringify(args[0]) : '{}';
            if (argsJson === undefined) argsJson = '{}';
            var resultJson = globalThis.__hostrunInvokeTool(toolPath, argsJson);
            return resultJson !== undefined && resultJson !== '' ? JSON.parse(resultJson) : undefined;
          }
        });
      })([]);
      Object.defineProperty(globalThis, 'eval', {
        value: undefined,
        writable: false,
        configurable: false
      });
      Object.defineProperty(Array.prototype, 'containing', {
        value: function containing(needle) {
          return this.filter(function(value) {
            return typeof value === 'string' && value.indexOf(String(needle)) !== -1;
          });
        },
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

function installToolInvoker(
  context: QuickJSContext,
  options: HostrunSessionOptions,
): void {
  const invoker = context.newFunction(
    "__hostrunInvokeTool",
    (toolPathHandle: QuickJSHandle, argsJsonHandle: QuickJSHandle) => {
      const toolPath = context.getString(toolPathHandle);
      const argsJson = context.getString(argsJsonHandle);

      try {
        const result = invokeCapability(toolPath, argsJson, options);
        return context.newString(result);
      } catch (error) {
        throw new Error(error instanceof Error ? error.message : String(error));
      }
    },
  );

  context.setProp(context.global, "__hostrunInvokeTool", invoker);
  invoker.dispose();
}

function invokeCapability(
  toolPath: string,
  argsJson: string,
  options: HostrunSessionOptions,
): string {
  const capability = options.capabilities?.[toolPath];
  if (!capability) {
    throw new HostrunSessionError(`Unknown Hostrun capability: ${toolPath}`);
  }

  const args = parseArgs(argsJson);
  const approval = capability.describe(args);
  const decision = options.approve?.(approval) ?? { type: "approve" };

  if (decision.type === "deny") {
    throw new HostrunSessionError(decision.reason);
  }

  if (decision.type === "pending") {
    throw new HostrunSessionError(
      `${APPROVAL_REQUIRED_PREFIX}${JSON.stringify(approval)}`,
    );
  }

  const result = capability.invoke(args);
  return result === undefined ? "" : JSON.stringify(result);
}

function parseArgs(argsJson: string): Record<string, unknown> {
  const parsed = JSON.parse(argsJson || "{}") as unknown;
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw new HostrunSessionError(
      "Hostrun capability arguments must be an object",
    );
  }

  return parsed as Record<string, unknown>;
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

function parsePendingApproval(error: unknown): HostrunApprovalRequest | null {
  const message = formatError(error);
  if (!message.startsWith(APPROVAL_REQUIRED_PREFIX)) {
    return null;
  }

  const approvalJson = message.slice(APPROVAL_REQUIRED_PREFIX.length);
  return JSON.parse(approvalJson) as HostrunApprovalRequest;
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
