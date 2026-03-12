<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  let { onClose }: { onClose: () => void } = $props();

  let engineTestStatus = $state("Unknown");
  let gimpStatus = $state("Unknown");
  let engineTestResult = $state("");
  let toolsListResult = $state("");
  let actionLog = $state<string[]>([]);

  function logAction(msg: string) {
    actionLog = [msg, ...actionLog].slice(0, 20);
  }

  async function testEngine() {
    engineTestStatus = "Checking\u2026";
    try {
      const healthy = await invoke<boolean>("engine_health");
      engineTestResult = healthy ? "Engine is healthy" : "Engine not ready";
      engineTestStatus = healthy ? "Connected" : "Offline";
    } catch (e) {
      engineTestResult = String(e);
      engineTestStatus = "Error";
    }
  }

  async function listTools() {
    gimpStatus = "Checking\u2026";
    try {
      const result = await invoke<any>("mcp_list_tools");
      toolsListResult = JSON.stringify(result, null, 2);
      gimpStatus = "Connected";
    } catch (e) {
      toolsListResult = String(e);
      gimpStatus = "Disconnected";
    }
  }

  async function runDrawTestLine() {
    logAction("Draw test line\u2026");
    try {
      await invoke("macro_draw_line", { x1: 50, y1: 50, x2: 200, y2: 200 });
      logAction("Line OK");
    } catch (e) { logAction("Failed: " + String(e)); }
  }

  async function runCropSquare() {
    logAction("Crop square\u2026");
    try {
      await invoke("macro_crop_square");
      logAction("Crop OK");
    } catch (e) { logAction("Failed: " + String(e)); }
  }

  async function runResize1024() {
    logAction("Resize to 1024w\u2026");
    try {
      await invoke("macro_resize", { width: 1024 });
      logAction("Resize OK");
    } catch (e) { logAction("Failed: " + String(e)); }
  }
</script>

<aside class="devtools">
  <div class="devtools-header">
    <span>Developer Tools</span>
    <button class="icon-btn" onclick={onClose}>✕</button>
  </div>

  <details class="dev-section">
    <summary>Engine · <em>{engineTestStatus}</em></summary>
    <button class="dev-btn" onclick={testEngine}>Test Engine</button>
    {#if engineTestResult}<pre>{engineTestResult}</pre>{/if}
  </details>

  <details class="dev-section">
    <summary>GIMP MCP · <em>{gimpStatus}</em></summary>
    <button class="dev-btn" onclick={listTools}>Refresh Tools</button>
    {#if toolsListResult}<pre>{toolsListResult}</pre>{/if}
  </details>

  <details class="dev-section" open>
    <summary>Quick Actions</summary>
    <div class="dev-actions">
      <button class="dev-btn" onclick={runDrawTestLine}>Line</button>
      <button class="dev-btn" onclick={runCropSquare}>Crop</button>
      <button class="dev-btn" onclick={runResize1024}>1024w</button>
    </div>
    {#if actionLog.length > 0}
      <pre class="action-log">{actionLog.join("\n")}</pre>
    {/if}
  </details>
</aside>

<style>
  .devtools {
    position: absolute;
    top: 57px;
    right: 0;
    bottom: 0;
    width: 280px;
    background: #fff;
    border-left: 1px solid #e0e0e8;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    z-index: 10;
    box-shadow: -4px 0 16px rgba(0,0,0,0.06);
  }
  .devtools-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 14px;
    border-bottom: 1px solid #eee;
    font-size: 13px;
    font-weight: 600;
    color: #333;
    flex-shrink: 0;
  }
  .icon-btn {
    width: 32px;
    height: 32px;
    border: 1px solid #e0e0e0;
    border-radius: 8px;
    background: #fafafa;
    font-size: 15px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #666;
    transition: background 0.15s, color 0.15s;
    flex-shrink: 0;
  }
  .icon-btn:hover { background: #f0f0f0; color: #333; }
  .dev-section {
    border-bottom: 1px solid #f0f0f0;
    font-size: 12px;
  }
  .dev-section > summary {
    padding: 10px 14px;
    cursor: pointer;
    user-select: none;
    color: #444;
    font-weight: 500;
    list-style: none;
  }
  .dev-section > summary::-webkit-details-marker { display: none; }
  .dev-section > summary::before { content: "▶ "; font-size: 9px; }
  .dev-section[open] > summary::before { content: "▼ "; }
  .dev-section em { font-style: normal; color: #888; font-weight: 400; }
  .dev-section > :not(summary) { padding: 0 14px 10px; }
  .dev-btn {
    margin: 4px 0;
    padding: 5px 10px;
    font-size: 12px;
    border-radius: 6px;
    border: 1px solid #d0d0d8;
    background: #fafafa;
    cursor: pointer;
    transition: background 0.1s;
  }
  .dev-btn:hover { background: #f0f0f0; }
  .dev-actions { display: flex; gap: 6px; flex-wrap: wrap; padding-top: 4px; }
  pre {
    font-size: 10px;
    background: #f4f4f8;
    border-radius: 6px;
    padding: 8px;
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-all;
    margin: 6px 0 0;
    color: #333;
  }
  .action-log { margin: 8px 0 0; }
</style>
