'use strict';

// Plan §9.4: "preload 不暴露通用 IPC、Node、process、文件、shell 或 URL 打开
// 能力". No business-state IPC surface has been designed yet (that lands
// with the real §9 navigation), so this preload intentionally exposes
// nothing via contextBridge — an empty, explicit allowlist rather than a
// missing file, so the next increment that *does* need a bridge method has
// to add it here deliberately instead of finding Node already reachable
// from the renderer by omission.
