import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { test, expect, visit, expandLog } from "./harness/browser.ts";
import { flowSteps, newTask, state } from "./harness/fixture.ts";

// 面板操作控件样式统一：全部浏览器断言都在这个新文件里，不改已有 spec。

const box = async (l: import("@playwright/test").Locator) =>
  (await l.boundingBox())!;

test("任务详情操作区：所有操作等高同行，「取消」是危险按钮", async ({
  page,
}) => {
  const f = state(),
    slug = "action-controls-run";
  flowSteps(f.root, slug, "feature", { holdFirstStep: true });
  const id = newTask(f.root, slug);
  try {
    await visit(page, `/tasks/${id}`);
    const ops = page
      .getByTestId("task-ops")
      .or(page.locator("section.task-ops"))
      .first()
      .locator("[data-op]");
    await expect(ops.filter({ hasText: "取消" }).first()).toBeVisible({
      timeout: 60_000,
    });
    const boxes = await Promise.all((await ops.all()).map(box));
    expect(boxes.length).toBeGreaterThanOrEqual(3);
    for (const b of boxes) {
      expect(b.height).toBeLessThanOrEqual(32);
      expect(Math.abs(b.height - boxes[0].height)).toBeLessThanOrEqual(1);
      expect(Math.abs(b.y - boxes[0].y)).toBeLessThanOrEqual(1);
    }
    const cancel = page.locator("[data-op=cancel]").first();
    const style = (l: import("@playwright/test").Locator) =>
      l.evaluate((el) => {
        const s = getComputedStyle(el);
        return {
          border:
            parseFloat(s.borderTopWidth) > 0 && s.borderTopStyle !== "none",
          bg: s.backgroundColor,
          radius: parseFloat(s.borderTopLeftRadius),
        };
      });
    const c = await style(cancel),
      other = await style(page.locator("[data-op=pause]").first());
    expect(
      c.border || c.bg !== "rgba(0, 0, 0, 0)",
      "取消需要可见边框或底色",
    ).toBe(true);
    expect(c.radius).toBeGreaterThanOrEqual(12);
    expect(Math.abs(c.radius - other.radius)).toBeLessThanOrEqual(1);
    // 取消确认：确认与返回同一行、等高
    await cancel.click();
    const confirm = page.getByRole("button", {
      name: "确认取消这个任务",
      exact: true,
    });
    await expect(confirm).toBeVisible();
    const back = page.locator(".task-ops").getByText("返回", { exact: true });
    const [cb, bb] = [await box(confirm), await box(back)];
    expect(Math.abs(cb.y - bb.y)).toBeLessThanOrEqual(1);
    expect(Math.abs(cb.height - bb.height)).toBeLessThanOrEqual(1);
    expect(cb.height).toBeLessThanOrEqual(32);
  } finally {
    // 只放行、不取消：layout.spec 写死了「已取消」列的条数，这里不能多造已取消任务。
    writeFileSync(join(f.root, `release-${slug}-1`), "x");
  }
});

// 排除清单（自带样式的专用控件）：页签/开关/单选(role)、纯图标或单字符按钮（步进器加减、图标）、
// 分段选择与开关类名、日志与时间线的展开三角、顶栏搜索入口（.shell-topbar-title，命令面板触发器）。
const SPECIAL =
  "[role=tab],[role=switch],[role=radio],[role=menuitemradio],[class*=seg],[class*=switch],[class*=step],[class*=tab],[class*=icon],[class*=chev],[class*=caret],[class*=toggle],[class*=disclosure],.shell-topbar-title";

test("全站清扫：每个可见按钮只有 30 或 38px 两种高度、圆角不小于 12px", async ({
  page,
}) => {
  const f = state(),
    slug = "action-controls-sweep";
  flowSteps(f.root, slug, "feature", { holdFirstStep: true });
  const id = newTask(f.root, slug);
  const bad: string[] = [];
  const sweep = async (label: string) => {
    const found = await page.locator("button:visible").evaluateAll(
      (els, special) =>
        els
          .filter(
            (el) =>
              !el.matches(special) &&
              !el.closest(special) &&
              (el.textContent ?? "").trim().length > 1,
          )
          .map((el) => {
            const r = el.getBoundingClientRect(),
              s = getComputedStyle(el);
            return {
              text: (el.textContent ?? "").trim().slice(0, 12),
              cls: el.className,
              h: Math.round(r.height * 10) / 10,
              radius: parseFloat(s.borderTopLeftRadius),
            };
          }),
      SPECIAL,
    );
    for (const b of found)
      if (
        !(Math.abs(b.h - 30) <= 1 || Math.abs(b.h - 38) <= 1) ||
        b.radius < 12
      )
        bad.push(
          `${label} 「${b.text}」 class="${b.cls}" 高${b.h} 圆角${b.radius}`,
        );
  };
  try {
    const pages: [string, string][] = [
      ["总览", "/"],
      ["项目看板", `/projects/${f.projectId}`],
      ["Loop 配置", "/loop"],
      ["技能", "/skills"],
      ["环境", "/env"],
      ["新建任务", `/projects/${f.projectId}/tasks/new`],
      ["添加项目", "/projects/new"],
      ["项目设置", `/projects/${f.projectId}/settings`],
    ];
    for (const [label, path] of pages) {
      await visit(page, path);
      await sweep(label);
    }
    await visit(page, `/tasks/${id}`);
    await expect(page.locator("[data-op=cancel]").first()).toBeVisible({
      timeout: 60_000,
    });
    await expandLog(page).catch(() => {});
    await sweep("任务详情");
    expect([...new Set(bad)], "落在兜底样式上的按钮").toEqual([]);
  } finally {
    // 只放行、不取消：layout.spec 写死了「已取消」列的条数，这里不能多造已取消任务。
    writeFileSync(join(f.root, `release-${slug}-1`), "x");
  }
});
