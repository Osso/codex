declare const process: {
  argv: string[];
  exitCode: number | undefined;
  stdin: AsyncIterable<unknown>;
  stdout: {
    write: (value: string) => void;
  };
  stderr: {
    write: (value: string) => void;
  };
};
