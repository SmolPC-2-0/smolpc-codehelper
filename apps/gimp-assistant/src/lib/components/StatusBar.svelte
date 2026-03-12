<script lang="ts">
  let {
    engineStatus,
    isConnected,
    imageInfo,
  }: {
    engineStatus: "unknown" | "ready" | "offline";
    isConnected: boolean;
    imageInfo: string;
  } = $props();
</script>

<div class="status-pill" class:connected={engineStatus === "ready"} class:offline={engineStatus === "offline"}>
  <span class="status-dot"></span>
  <span class="status-label">
    {engineStatus === "ready" ? "Engine ready" : engineStatus === "offline" ? "Engine offline" : "Engine..."}
  </span>
</div>
<div class="status-pill" class:connected={isConnected}>
  <span class="status-dot"></span>
  <span class="status-label">
    {isConnected ? (imageInfo || "GIMP connected") : "GIMP offline"}
  </span>
</div>

<style>
  .status-pill {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border-radius: 20px;
    background: #f5f5f5;
    border: 1px solid #e0e0e0;
    font-size: 12px;
    color: #888;
    transition: background 0.2s, color 0.2s;
  }
  .status-pill.connected {
    background: #f0faf3;
    border-color: #b8e6c6;
    color: #2a7a45;
  }
  .status-pill.offline {
    background: #fef2f2;
    border-color: #fecaca;
    color: #b91c1c;
  }
  .status-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #ccc;
    flex-shrink: 0;
    transition: background 0.2s;
  }
  .status-pill.connected .status-dot { background: #34c759; }
  .status-pill.offline .status-dot { background: #ef4444; }
  .status-label { white-space: nowrap; max-width: 140px; overflow: hidden; text-overflow: ellipsis; }
</style>
