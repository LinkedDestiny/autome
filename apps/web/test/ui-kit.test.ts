import { createElement as h } from 'react';
import { renderToStaticMarkup as render } from 'react-dom/server';
import { expect, it } from 'vitest';
import { Card, type CardColor } from '../app/components/ui/card.tsx';
import { Ribbon } from '../app/components/ui/ribbon.tsx';
import { Pill, type PillColor } from '../app/components/ui/pill.tsx';
import { Tile } from '../app/components/ui/tile.tsx';
import { Progress } from '../app/components/ui/progress.tsx';
import { Button } from '../app/components/ui/button.tsx';
import { Segmented, SegmentedTabs } from '../app/components/ui/segmented.tsx';
import { Switch } from '../app/components/ui/switch.tsx';
import { Stepper } from '../app/components/ui/stepper.tsx';
import { Input } from '../app/components/ui/input.tsx';
import { Check } from '../app/components/ui/check.tsx';
import { Icon } from '../app/components/icons.tsx';

it('卡片默认与六种颜色、双层图案、虚线均可组合', () => {
  expect(render(h(Card, null, '内容'))).toBe('<section class="card">内容</section>');
  for (const color of ['yellow', 'teal', 'purple', 'blue', 'green', 'red'] as CardColor[]) {
    expect(render(h(Card, { color, patterned: true }))).toContain(`card card-${color} card-pat`);
  }
  expect(render(h(Card, { dashed: true }))).toContain('card card-dash');
});
it('丝带四色与默认青色、小号均有正面内容', () => {
  for (const color of ['yellow', 'blue', 'green', 'brown', 'teal'] as const) {
    expect(render(h(Ribbon, { color, small: true }, '标题'))).toContain(`class="ribbon ribbon-${color} ribbon-sm"><span>标题</span>`);
  }
});
it('胶囊默认描边与九种实心颜色', () => {
  expect(render(h(Pill, null, '状态'))).not.toContain('pill-fill');
  for (const color of ['teal', 'blue', 'purple', 'orange', 'pink', 'brown', 'green', 'yellow', 'red'] as PillColor[]) {
    expect(render(h(Pill, { color, filled: true }, '状态'))).toContain(`pill pill-${color} pill-fill`);
  }
});
it('数字块七色与附注', () => {
  for (const color of ['teal', 'yellow', 'blue', 'pink', 'purple', 'orange', 'green'] as const) {
    const html = render(h(Tile, { color, value: 12, label: '数量' }, '今天'));
    expect(html).toContain(`tile tile-${color}`); expect(html).toContain('数量<b>12</b><small>今天</small>');
  }
});
it('数字缺失显示横线，真实零值不算缺失', () => {
  for (const value of [null, undefined]) {
    const html = render(h(Tile, { color: 'blue', value, label: '数量' }));
    expect(html).toContain('data-missing="true"'); expect(html).toContain('<b>—</b>');
  }
  const zero = render(h(Tile, { color: 'blue', value: 0, label: '数量' }));
  expect(zero).toContain('<b>0</b>'); expect(zero).not.toContain('data-missing');
});
it('进度两端夹取及合法小数，SVG 属性与无障碍值一致', () => {
  for (const [value, expected] of [[-10, 0], [0, 0], [42.5, 42.5], [100, 100], [140, 100]]) {
    const html = render(h(Progress, { value, label: '进度' }));
    expect(html).toContain(`aria-valuenow="${expected}"`); expect(html).toContain(`<rect width="${expected}"`);
    expect(html).toContain('role="progressbar"'); expect(html).toContain('aria-valuemin="0"'); expect(html).toContain('aria-valuemax="100"');
    expect(html).not.toContain('style=');
  }
});
it('进度非有限数不产生无效 SVG 值', () => {
  for (const [value, expected] of [[NaN, 0], [-Infinity, 0], [Infinity, 100]]) {
    expect(render(h(Progress, { value, label: '进度' }))).toContain(`aria-valuenow="${expected}"`);
  }
});
it('只有主按钮带主类，次要与文字按钮没有立体主类', () => {
  for (const variant of ['default', 'primary', 'text'] as const) {
    const html = render(h(Button, { variant, small: true }, '保存'));
    expect(html.includes('btn-primary')).toBe(variant === 'primary'); expect(html).toContain('btn-sm');
  }
});
it('按钮默认不提交表单，可显式提交和禁用', () => {
  expect(render(h(Button, null, '操作'))).toContain('type="button"');
  expect(render(h(Button, { type: 'submit', disabled: true }, '保存'))).toMatch(/disabled="".*type="submit"/);
});
it('分段选择使用同名原生 radio，只有当前值选中', () => {
  const html = render(h(Segmented, { name: 'scope', label: '范围', value: 'global', onChange: () => {}, options: [{ value: 'global', label: '全局' }, { value: 'project', label: '项目' }] }));
  expect(html.match(/type="radio"/g)).toHaveLength(2); expect(html.match(/name="scope"/g)).toHaveLength(2);
  expect(html.match(/checked=""/g)).toHaveLength(1); expect(html).toContain('role="radiogroup"');
});
it('分段选择单元素、空选项与禁用选项', () => {
  const props = { name: 'scope', label: '范围', value: 'a', onChange: () => {} };
  expect(render(h(Segmented, { ...props, options: [] }))).not.toContain('<input');
  expect(render(h(Segmented, { ...props, options: [{ value: 'a', label: '全局', disabled: true }] }))).toContain('disabled=""');
  expect(render(h(Segmented, { ...props, disabled: true, options: [{ value: 'a', label: '全局' }] }))).toContain('disabled=""');
});
it('开关保持原生 checkbox 与 switch 角色、开关两态及禁用', () => {
  for (const checked of [true, false]) {
    const html = render(h(Switch, { label: '启用', checked, onChange: () => {}, disabled: true, name: 'enabled' }));
    expect(html).toContain('type="checkbox"'); expect(html).toContain('role="switch"'); expect(html.includes('checked=""')).toBe(checked);
    expect(html).toContain('disabled=""'); expect(html).toContain('启用');
  }
});
it('步进器最小、最大、中间、单元素域对应按钮禁用', () => {
  for (const [value, min, max, disabled] of [[1, 1, 3, 1], [2, 1, 3, 0], [3, 1, 3, 1], [1, 1, 1, 2]]) {
    const html = render(h(Stepper, { value, min, max, label: '数量', name: 'count', onChange: () => {} }));
    const buttons = html.match(/<button[^>]+>/g)!;
    expect(buttons.filter(tag => tag.includes('disabled'))).toHaveLength(disabled);
    expect(buttons[0].includes('disabled')).toBe(value <= min); expect(buttons[1].includes('disabled')).toBe(value >= max);
    expect(html).toContain('type="number"'); expect(html).toContain(`min="${min}"`); expect(html).toContain(`max="${max}"`);
  }
});
it('步进器整体禁用涵盖两个按钮和数值输入', () => {
  expect(render(h(Stepper, { value: 2, min: 1, max: 3, label: '数量', disabled: true, onChange: () => {} })).match(/disabled=""/g)).toHaveLength(3);
});
it('输入框保留表单约束与空值', () => {
  const html = render(h(Input, { name: 'model', 'aria-label': '模型', defaultValue: '', required: true, maxLength: 80 }));
  for (const fragment of ['class="inp"', 'name="model"', 'value=""', 'required=""', 'maxLength="80"']) expect(html).toContain(fragment);
});
it('勾选块禁用原因可追溯，保留外部描述且两态可渲染', () => {
  const html = render(h(Check, { label: '技能', disabled: true, reason: '当前工具不可用', 'aria-describedby': 'extra', defaultChecked: true }));
  const id = html.match(/<label class="chk">[\s\S]*<small id="([^"]+)" class="chk-reason" title="当前工具不可用">当前工具不可用</)![1];
  expect(html).toContain('</small></label>');
  expect(html).toContain(`aria-describedby="extra ${id}"`); expect(html).toContain('当前工具不可用'); expect(html).toContain('disabled=""'); expect(html).toContain('checked=""');
  const enabled = render(h(Check, { label: '技能' })); expect(enabled).not.toContain('aria-describedby'); expect(enabled).not.toContain('disabled');
});
it('十个图标均独立渲染 SVG，无 sprite 和文本图标', () => {
  for (const name of ['home', 'plus', 'gear', 'bell', 'search', 'hand', 'check', 'route', 'bag', 'leaf'] as const) {
    const html = render(h(Icon, { name }));
    expect(html).toContain('viewBox="0 0 24 24"'); expect(html).toContain('aria-hidden="true"'); expect(html).toContain('<path'); expect(html).not.toContain('<use');
  }
});

it('步进器真实回调：增减、手输夹取、空值拒绝与整数域', () => {
  const changes: number[] = [];
  let element: ReturnType<typeof Stepper>;
  function Capture() {
    element = Stepper({ value: 2, min: 1, max: 3, label: '数量', onChange: value => changes.push(value) });
    return element;
  }
  render(h(Capture));
  const [minus, input, plus] = element!.props.children[1].props.children;
  minus.props.onClick(); plus.props.onClick();
  for (const valueAsNumber of [-10, 140, NaN, Infinity, 1.4, 2.6, 0, 3]) input.props.onChange({ currentTarget: { valueAsNumber } });
  expect(changes).toEqual([1, 3, 1, 3, 1, 3, 1, 3]);
});
it('分段选择两个方向触发对应值的回调', () => {
  const changes: string[] = [];
  const options = [{ value: 'global', label: '全局' }, { value: 'project', label: '项目' }];
  for (const value of ['global', 'project']) {
    const element = Segmented({ name: 'scope', label: '范围', value, options, onChange: next => changes.push(next) });
    const target = element.props.children[value === 'global' ? 1 : 0];
    target.props.children[0].props.onChange();
  }
  expect(changes).toEqual(['project', 'global']);
});
it('开关与勾选的原生变更回调保留两个方向', () => {
  const changes: boolean[] = [];
  const onChange = (event: { currentTarget: { checked: boolean } }) => changes.push(event.currentTarget.checked);
  const toggle = Switch({ label: '启用', onChange });
  let check: ReturnType<typeof Check>;
  function Capture() { check = Check({ label: '技能', onChange }); return check; }
  render(h(Capture));
  for (const checked of [true, false]) {
    toggle.props.children[0].props.onChange({ currentTarget: { checked } });
    check!.props.children.props.children[0].props.onChange({ currentTarget: { checked } });
  }
  expect(changes).toEqual([true, true, false, false]);
});

it('链接分段保留导航语义、类名、空与单元素子项', () => {
  expect(render(h(SegmentedTabs, { label: '范围', children: null }))).toContain('aria-label="范围"');
  const html = render(h(SegmentedTabs, { label: '范围', className: 'scope-tabs', children: h('a', { href: '/loop', 'aria-current': 'page' }, '全局') }));
  expect(html).toBe('<nav class="seg scope-tabs" aria-label="范围"><a href="/loop" aria-current="page">全局</a></nav>');
});
