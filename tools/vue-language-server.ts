import { execFileSync } from "node:child_process";
import { realpathSync } from "node:fs";
import { createRequire } from "node:module";

let vueExecutable = execFileSync(
    "mise",
    ["which", "vue-language-server", "--tool", "npm:@vue/language-server@2.2.12"],
    { encoding: "utf8" },
).trim();
let typescriptExecutable = execFileSync(
    "mise",
    ["which", "tsc", "--tool", "npm:typescript@5.9.3"],
    { encoding: "utf8" },
).trim();
let vueEntry = createRequire(realpathSync(vueExecutable)).resolve("@vue/language-server");
let requireVue = createRequire(vueEntry);
let requireTypescript = createRequire(realpathSync(typescriptExecutable));
let typescript = requireTypescript("typescript");
let { createConnection, createServer } = requireVue("@volar/language-server/node");
let { initialize } = requireVue("@vue/language-server");
let connection = createConnection();
let server = createServer(connection);

connection.onInitialize((parameters: unknown) =>
    initialize(server, parameters, typescript, undefined),
);
connection.onInitialized(server.initialized);
connection.onShutdown(server.shutdown);
connection.listen();
