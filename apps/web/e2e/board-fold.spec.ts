import { test, expect, visit } from "./harness/browser.ts";
import { state } from "./harness/fixture.ts";

// 看板「已完成」列折叠入口被压扁：统一按钮（.btn）没有禁止被弹性容器收缩，
// 列内容溢出时被压到文字行高（约 21px）。
// 让列溢出的办法：共享夹具里本来就有 30 个已取消任务，点开「已取消」展开，
// 并把视口高度调小到 1512×420。不新建任务，避免污染共享夹具
// （layout.spec 断言已取消恰为 30）。

test("已完成列溢出时折叠入口仍保持统一按钮高度", async ({ page }) => {
  const f = state();
  await page.setViewportSize({ width: 1512, height: 420 });
  await visit(page, `/projects/${encodeURIComponent(f.projectId)}`);
  const body = page.locator(
    '.board-column[data-column="done"] .board-column-body',
  );
  await page.getByTestId("board-cancelled-toggle").click();
  await expect(page.getByTestId("board-cancelled")).toBeVisible();
  // 先确认确实到达溢出场景，否则用例是空过的
  const m = await body.evaluate((el) => ({
    scroll: el.scrollHeight,
    client: el.clientHeight,
  }));
  expect(m.scroll, "已完成列必须溢出").toBeGreaterThan(m.client);
  const btns = await body.locator(".btn").all();
  expect(btns.length).toBeGreaterThanOrEqual(1);
  for (const b of btns) {
    if (!(await b.isVisible())) continue;
    const h = await b.evaluate((el) => el.getBoundingClientRect().height);
    const near = (t: number) => Math.abs(h - t) <= 1;
    expect(near(30) || near(38), `按钮高度 ${h}px 应为 30 或 38`).toBe(true);
  }
});
