export interface PickerWindow { autome?: { pickDirectory?: () => Promise<string | null> } }
export function nativePicker(win: PickerWindow) { return typeof win.autome?.pickDirectory === 'function' ? win.autome.pickDirectory.bind(win.autome) : undefined; }
