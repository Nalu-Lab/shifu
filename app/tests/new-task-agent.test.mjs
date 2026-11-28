// Run with SHIFU_TEST_CONNECTOR pointing to the shared browser connector.
import { createRequire } from "node:module";
import { expect, it } from "vitest";
import { createServer } from "vite";

const connector = process.env.SHIFU_TEST_CONNECTOR;

it.skipIf(!connector)("keeps the clicked agent through task creation and the next opening", async () => {
  const browser = await createRequire(import.meta.url)(connector).connect();
  const server = await createServer({
    configFile: false,
    server: { host: "127.0.0.1", port: 0, hmr: false },
    logLevel: "error",
  });
  try {
    await server.listen();
    const page = await browser.newPage();
    await page.route("**/picker-test", (route) => route.fulfill({
      contentType: "text/html",
      body: '<button id="new">New task</button><output></output>',
    }));
    await page.goto(server.resolvedUrls.local[0] + "picker-test");
    await page.evaluate(`(async () => {
      window.__TAURI_INTERNALS__ = { invoke: async (cmd) => {
        if (cmd === "permissions_get") return "own";
        if (cmd === "recent_dirs") return [];
        throw new Error(cmd);
      } };
      await import("/src/style.css");
      const { showNewTaskPopover } = await import("/src/newpopover.ts");
      const anchor = document.getElementById("new");
      anchor.onclick = () => showNewTaskPopover(anchor, {
        projects: ["/project"],
        create: async (_project, _name, kind) => { document.querySelector("output").value = kind; },
      });

      // macOS WebKit drops focus on a mouse press on a button. Reproduce that
      // default action in Chromium too, after the control handles mousedown.
      document.addEventListener("mousedown", (event) => {
        if (!event.defaultPrevented && event.target.closest('[role="option"]')) {
          document.activeElement.blur();
        }
      });
    })()`);

    const toggle = page.locator(".agent-dropdown-button");
    await page.locator("#new").click();
    expect(await toggle.innerText()).toBe("Claude");
    for (const [label, kind] of [["Codex", "codex"], ["Kimi", "kimi"]]) {
      await toggle.click();
      await page.getByRole("option", { name: label, exact: true }).click();
      expect(await toggle.innerText()).toBe(label);
      await page.getByRole("button", { name: "建立", exact: true }).click();
      expect(await page.locator("output").textContent()).toBe(kind);
      await page.locator("#new").click();
      expect(await toggle.innerText()).toBe(label);
    }
  } finally {
    await browser.close();
    await server.close();
  }
}, 30000);
