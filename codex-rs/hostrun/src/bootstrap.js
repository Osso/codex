globalThis.ctx = globalThis.ctx ?? {};
globalThis.__hostrun_console = [];

globalThis.__hostrun_formatConsoleValue = function (value) {
  if (typeof value === "string") {
    return value;
  }
  try {
    return JSON.stringify(value);
  } catch (_error) {
    return String(value);
  }
};

globalThis.__hostrun_consolePush = function (level, args) {
  globalThis.__hostrun_console.push({
    level,
    message: Array.from(args).map(globalThis.__hostrun_formatConsoleValue).join(" ")
  });
};

globalThis.console = {
  log: function (...args) { globalThis.__hostrun_consolePush("log", args); },
  info: function (...args) { globalThis.__hostrun_consolePush("info", args); },
  warn: function (...args) { globalThis.__hostrun_consolePush("warn", args); },
  error: function (...args) { globalThis.__hostrun_consolePush("error", args); },
  debug: function (...args) { globalThis.__hostrun_consolePush("debug", args); }
};

if (!Array.prototype.containing) {
  Object.defineProperty(Array.prototype, "containing", {
    value: function (needle) {
      return this.filter((value) => String(value).includes(String(needle)));
    },
    configurable: true,
    writable: true
  });
}

globalThis.__hostrun_invokeCapability = function (path, payload) {
  const response = JSON.parse(globalThis.__hostrun_invokeTool(path, JSON.stringify(payload ?? {})));
  if (response.type === "needs_approval") {
    throw new Error("__HOSTRUN_APPROVAL_REQUIRED__:" + JSON.stringify(response.approval));
  }
  if (response.type === "denied") {
    throw new Error(response.reason);
  }
  return response.value;
};

globalThis.__hostrun_toolProxy = function (path) {
  return new Proxy(function () {}, {
    get(_target, property) {
      return globalThis.__hostrun_toolProxy(path ? path + "." + String(property) : String(property));
    },
    apply(_target, _thisArg, args) {
      const payload = args.length > 0 ? args[0] : {};
      return globalThis.__hostrun_invokeCapability(path, payload);
    }
  });
};

globalThis.tools = globalThis.__hostrun_toolProxy("");

globalThis.fs = {
  write: function (path, content) {
    return globalThis.__hostrun_invokeCapability("fs.write", { path, content });
  },
  read: function (path) {
    return globalThis.__hostrun_invokeCapability("fs.read", { path });
  },
  exists: function (path) {
    return globalThis.__hostrun_invokeCapability("fs.exists", { path });
  },
  remove: function (path) {
    return globalThis.__hostrun_invokeCapability("fs.remove", { path });
  }
};

globalThis.rclone = {
  deletefile: function (target) {
    return globalThis.__hostrun_invokeCapability("rclone.deletefile", { target });
  }
};

globalThis.__hostrun_commandBuilder = function (program, args) {
  const state = {
    program,
    args: Array.from(args)
  };
  const builder = {
    program: state.program,
    args: state.args,
    run: function () {
      return globalThis.__hostrun_invokeCapability("cli." + state.program, state);
    },
    toJSON: function () {
      return { ...state };
    }
  };
  const streamHandle = function (name) {
    return {
      stream: name,
      command: state,
      capture: function () {
        state[name] = { type: "capture" };
        return builder;
      },
      toFile: function (path) {
        state[name] = { type: "file", path };
        return builder;
      },
      toJSON: function () {
        return { stream: name, command: { program: state.program, args: state.args } };
      }
    };
  };
  builder.stdout = streamHandle("stdout");
  builder.stderr = streamHandle("stderr");
  builder.stderr.toStdout = function () {
    state.stderr = { type: "stdout" };
    return builder;
  };
  builder.combined = {
    capture: function () {
      state.combined = { type: "capture" };
      return builder;
    },
    toFile: function (path) {
      state.combined = { type: "file", path };
      return builder;
    }
  };
  const stdin = function (source) {
    state.stdin = { type: "stream", source };
    return builder;
  };
  stdin.text = function (text) {
    state.stdin = { type: "text", text };
    return builder;
  };
  stdin.file = function (path) {
    state.stdin = { type: "file", path };
    return builder;
  };
  stdin.json = function (value) {
    state.stdin = { type: "json", value };
    return builder;
  };
  stdin.lines = function (lines) {
    state.stdin = { type: "lines", lines };
    return builder;
  };
  builder.stdin = stdin;
  return builder;
};

globalThis.__hostrun_cliProxy = function (path) {
  return new Proxy(function () {}, {
    get(_target, property) {
      return globalThis.__hostrun_cliProxy(path ? path + "." + String(property) : String(property));
    },
    apply(_target, _thisArg, args) {
      return globalThis.__hostrun_commandBuilder(path, args);
    }
  });
};

globalThis.cli = globalThis.__hostrun_cliProxy("");

globalThis.__hostrun_run = function (code) {
  globalThis.__hostrun_console = [];
  try {
    const value = (0, eval)(code);
    return JSON.stringify({
      type: "completed",
      executed: code,
      console: globalThis.__hostrun_console,
      value: value === undefined ? null : value
    });
  } catch (error) {
    const message = error && error.message ? String(error.message) : String(error);
    if (message.startsWith("__HOSTRUN_APPROVAL_REQUIRED__:")) {
      return message;
    }
    throw error;
  }
};
