/** Match leaked JavaScript values, not ordinary words containing their letters. */
export function dirtyText(text: string): string[] {
  return text.match(/\b(?:undefined|NaN|null)\b|\[object Object\]/g) ?? [];
}
