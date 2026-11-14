export const GUARD_LABELS: Record<string, string> = {
  audit_made_no_progress: '审计未推进里程碑', evidence_missing: '缺少本轮证据',
  impl_closed_milestone: '实现轮不得关闭里程碑', impl_had_no_target: '实现轮没有开放的里程碑',
  retro_not_one_line: '复盘记录应为一行', retro_line_too_long: '复盘记录超长',
  audit_edited_the_design: '审计修改了设计正文', design_doc_large: '设计文档过大', design_doc_grew: '设计文档增长过多',
  lessons_missing: '缺少经验记录', lessons_unparseable: '经验记录无法解析', brief_map_unreadable: '简报映射无法读取',
};
export const guardLabel = (code: string): string => Object.hasOwn(GUARD_LABELS, code) ? GUARD_LABELS[code] : code;
