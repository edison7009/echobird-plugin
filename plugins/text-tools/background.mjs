const host = window.echobird;
async function format(input, { signal }) {
  const settings = await host.configuration.get();
  if (signal.aborted) throw new Error("Cancelled");
  const text = String(input?.text ?? "").trim();
  return (
    settings.prefix +
    (settings["collapse-spaces"] ? text.replace(/[ \t]+/gu, " ") : text)
  );
}
const command = host.commands.register("format", async (input, context) => {
  const text = await format(input, context);
  if (context.signal.aborted) throw new Error("Cancelled");
  const count = await host.storage.get("invocations");
  await host.storage.set(
    "invocations",
    (typeof count === "number" ? count : 0) + 1,
  );
  await host.events.emit("formatted", { length: text.length });
  return text;
});
const processor = host.text.register("clean-output", format);
host.lifecycle.onDispose(() => {
  command.dispose();
  processor.dispose();
});
