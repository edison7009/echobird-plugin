interface EchoBirdDisposable {
  dispose(): void | Promise<void>;
}
interface EchoBirdHandlerContext {
  readonly signal: AbortSignal;
}
type EchoBirdHandler = (
  input: unknown,
  context: EchoBirdHandlerContext,
) => unknown | Promise<unknown>;

interface EchoBirdPageHost {
  readonly apiVersion: "0.1-draft";
  readonly context: Readonly<{
    locale: string;
    modelName: string | null;
    pluginId: string;
    commands: readonly string[];
    textProcessors: readonly string[];
  }>;
  /** Requires model.request. Uses the model selected by the user at launch. */
  generateText(prompt: string): Promise<string>;
  /** Requires clipboard.write. Writes at most 1 MiB; never reads the clipboard. */
  writeClipboardText(text: string): Promise<void>;
  readonly configuration: {
    get<T = Record<string, boolean | number | string>>(
      key?: string,
    ): Promise<T>;
    update(key: string, value: boolean | number | string): Promise<void>;
    reset(key: string): Promise<void>;
  };
  readonly storage: {
    get<T = unknown>(key: string): Promise<T | null>;
    set(key: string, value: unknown): Promise<void>;
    delete(key: string): Promise<void>;
  };
  readonly commands: {
    register(id: string, handler: EchoBirdHandler): EchoBirdDisposable;
    execute(id: string, input?: unknown): Promise<unknown>;
  };
  readonly text: {
    register(
      id: string,
      handler: (
        input: { text: string; stage: "voice.transcribed" | "voice.output" },
        context: EchoBirdHandlerContext,
      ) => string | Promise<string>,
    ): EchoBirdDisposable;
  };
  readonly events: {
    on(
      topic: string,
      handler: (value: unknown) => void | Promise<void>,
    ): Promise<EchoBirdDisposable>;
    emit(topic: string, value?: unknown): Promise<void>;
  };
  readonly lifecycle: { onDispose(handler: () => void): EchoBirdDisposable };
  close(): Promise<void>;
  minimize(): Promise<void>;
}

interface Window {
  readonly echobird?: EchoBirdPageHost;
}
