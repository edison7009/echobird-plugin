import readline from "node:readline";

export async function serveAccountOperations(operations) {
  const output = process.stdout.write.bind(process.stdout);
  console.log =
    console.info =
    console.debug =
      (...args) => console.error(...args);
  for await (const line of readline.createInterface({ input: process.stdin })) {
    const request = JSON.parse(line);
    if (
      request.jsonrpc !== "2.0" ||
      typeof request.id !== "string" ||
      typeof request.method !== "string" ||
      typeof request.params?.providerId !== "string"
    )
      throw new Error("Invalid account operation request");
    let response;
    try {
      if (
        !Object.hasOwn(operations, request.method) ||
        typeof operations[request.method] !== "function"
      )
        throw new Error("Unsupported account operation");
      const result = await operations[request.method](request.params.input, {
        providerId: request.params.providerId,
      });
      response = { jsonrpc: "2.0", id: request.id, result: result ?? null };
    } catch {
      response = {
        jsonrpc: "2.0",
        id: request.id,
        error: { code: -32000, message: "Account operation failed" },
      };
    }
    output(JSON.stringify(response) + "\n");
  }
}
