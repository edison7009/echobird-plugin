interface EchoBirdPageHost {
  readonly apiVersion: "0.1-draft";
  readonly context: Readonly<{
    locale: string;
    modelName: string | null;
    pluginId: string;
  }>;
  /** Requires model.request. Uses the model selected by the user at launch. */
  generateText(prompt: string): Promise<string>;
  /** Requires clipboard.write. Writes at most 1 MiB; never reads the clipboard. */
  writeClipboardText(text: string): Promise<void>;
  close(): Promise<void>;
  minimize(): Promise<void>;
}

interface Window {
  readonly echobird?: EchoBirdPageHost;
}
