// 标准 98 键布局
// 右列对齐：Del → BSp → \ → Enter → RShift（右边缘统一 15.00）
// 导航区 3×2：col 15.25~18.25
// 小键盘：col 18.75~22.75，从 Row 1 开始（下移一行）
// 方向键：倒 T 在导航区下方

export interface KeyPos {
  row: number
  col: number
  w: number
  h?: number
  label: string
  code: number
}

export const KEYBOARD_LAYOUT: KeyPos[] = [
  // ═══ Row 0: Esc + F1~F12 ┃ Del ┃ Ins Hm PU ═══
  { row: 0, col: 0.00,  w: 1.0, label: 'Esc', code: 27 },
  { row: 0, col: 1.25,  w: 1.0, label: 'F1',  code: 112 },
  { row: 0, col: 2.25,  w: 1.0, label: 'F2',  code: 113 },
  { row: 0, col: 3.25,  w: 1.0, label: 'F3',  code: 114 },
  { row: 0, col: 4.25,  w: 1.0, label: 'F4',  code: 115 },
  { row: 0, col: 5.50,  w: 1.0, label: 'F5',  code: 116 },
  { row: 0, col: 6.50,  w: 1.0, label: 'F6',  code: 117 },
  { row: 0, col: 7.50,  w: 1.0, label: 'F7',  code: 118 },
  { row: 0, col: 8.50,  w: 1.0, label: 'F8',  code: 119 },
  { row: 0, col: 9.75,  w: 1.0, label: 'F9',  code: 120 },
  { row: 0, col: 10.75, w: 1.0, label: 'F10', code: 121 },
  { row: 0, col: 11.75, w: 1.0, label: 'F11', code: 122 },
  { row: 0, col: 12.75, w: 1.0, label: 'F12', code: 123 },
  // 右列
  { row: 0, col: 14.00, w: 1.0, label: 'Del', code: 46 },
  // 导航区 Row 0
  { row: 0, col: 15.25, w: 1.0, label: 'Ins', code: 45 },
  { row: 0, col: 16.25, w: 1.0, label: 'Hm',  code: 36 },
  { row: 0, col: 17.25, w: 1.0, label: 'PU',  code: 33 },

  // ═══ Row 1: ` ~ = ┃ BSp ┃ Del End PD ┃ NL / * - ═══
  { row: 1, col: 0.00,  w: 1.0,  label: '`',    code: 192 },
  { row: 1, col: 1.00,  w: 1.0,  label: '1',    code: 49 },
  { row: 1, col: 2.00,  w: 1.0,  label: '2',    code: 50 },
  { row: 1, col: 3.00,  w: 1.0,  label: '3',    code: 51 },
  { row: 1, col: 4.00,  w: 1.0,  label: '4',    code: 52 },
  { row: 1, col: 5.00,  w: 1.0,  label: '5',    code: 53 },
  { row: 1, col: 6.00,  w: 1.0,  label: '6',    code: 54 },
  { row: 1, col: 7.00,  w: 1.0,  label: '7',    code: 55 },
  { row: 1, col: 8.00,  w: 1.0,  label: '8',    code: 56 },
  { row: 1, col: 9.00,  w: 1.0,  label: '9',    code: 57 },
  { row: 1, col: 10.00, w: 1.0,  label: '0',    code: 48 },
  { row: 1, col: 11.00, w: 1.0,  label: '-',    code: 189 },
  { row: 1, col: 12.00, w: 1.0,  label: '=',    code: 187 },
  // 右列
  { row: 1, col: 13.00, w: 2.0, label: 'BSp',  code: 8 },
  // 导航区 Row 1
  { row: 1, col: 15.25, w: 1.0, label: 'Del',  code: 46 },
  { row: 1, col: 16.25, w: 1.0, label: 'End',  code: 35 },
  { row: 1, col: 17.25, w: 1.0, label: 'PD',   code: 34 },
  // 小键盘 Row 1（下移一行，从这开始）
  { row: 1, col: 18.75, w: 1.0, label: 'NL',   code: 144 },
  { row: 1, col: 19.75, w: 1.0, label: '/',    code: 111 },
  { row: 1, col: 20.75, w: 1.0, label: '*',    code: 106 },
  { row: 1, col: 21.75, w: 1.0, label: '-',    code: 109 },
  // + 竖键在 Row 2 col 21.75

  // ═══ Row 2: Tab ~ ] ┃ \ ┃ 小键盘 7 8 9 + ═══
  { row: 2, col: 0.00,  w: 1.5, label: 'Tab', code: 9 },
  { row: 2, col: 1.50,  w: 1.0, label: 'Q',   code: 81 },
  { row: 2, col: 2.50,  w: 1.0, label: 'W',   code: 87 },
  { row: 2, col: 3.50,  w: 1.0, label: 'E',   code: 69 },
  { row: 2, col: 4.50,  w: 1.0, label: 'R',   code: 82 },
  { row: 2, col: 5.50,  w: 1.0, label: 'T',   code: 84 },
  { row: 2, col: 6.50,  w: 1.0, label: 'Y',   code: 89 },
  { row: 2, col: 7.50,  w: 1.0, label: 'U',   code: 85 },
  { row: 2, col: 8.50,  w: 1.0, label: 'I',   code: 73 },
  { row: 2, col: 9.50,  w: 1.0, label: 'O',   code: 79 },
  { row: 2, col: 10.50, w: 1.0, label: 'P',   code: 80 },
  { row: 2, col: 11.50, w: 1.0, label: '[',   code: 219 },
  { row: 2, col: 12.50, w: 1.0, label: ']',   code: 221 },
  // 右列
  { row: 2, col: 13.50, w: 1.5, label: '\\',   code: 220 },
  // 小键盘 Row 2
  { row: 2, col: 18.75, w: 1.0, label: '7', code: 103 },
  { row: 2, col: 19.75, w: 1.0, label: '8', code: 104 },
  { row: 2, col: 20.75, w: 1.0, label: '9', code: 105 },
  { row: 2, col: 21.75, w: 1.0, h: 2, label: '+', code: 107 },

  // ═══ Row 3: Caps ~ ' ┃ Enter ┃ 小键盘 4 5 6 + ═══
  { row: 3, col: 0.00,  w: 1.75, label: 'Caps', code: 20 },
  { row: 3, col: 1.75,  w: 1.0,  label: 'A',    code: 65 },
  { row: 3, col: 2.75,  w: 1.0,  label: 'S',    code: 83 },
  { row: 3, col: 3.75,  w: 1.0,  label: 'D',    code: 68 },
  { row: 3, col: 4.75,  w: 1.0,  label: 'F',    code: 70 },
  { row: 3, col: 5.75,  w: 1.0,  label: 'G',    code: 71 },
  { row: 3, col: 6.75,  w: 1.0,  label: 'H',    code: 72 },
  { row: 3, col: 7.75,  w: 1.0,  label: 'J',    code: 74 },
  { row: 3, col: 8.75,  w: 1.0,  label: 'K',    code: 75 },
  { row: 3, col: 9.75,  w: 1.0,  label: 'L',    code: 76 },
  { row: 3, col: 10.75, w: 1.0,  label: ';',    code: 186 },
  { row: 3, col: 11.75, w: 1.0,  label: "'",    code: 222 },
  // 右列
  { row: 3, col: 12.75, w: 2.25, label: 'Ent',  code: 13 },
  // 小键盘 Row 3
  { row: 3, col: 18.75, w: 1.0, label: '4', code: 100 },
  { row: 3, col: 19.75, w: 1.0, label: '5', code: 101 },
  { row: 3, col: 20.75, w: 1.0, label: '6', code: 102 },
  // + 竖键在 Row 2 跨到 Row 3，此处无独立键

  // ═══ Row 4: Shift ~ / ┃ RShift ┃ ↑ ┃ 小键盘 1 2 3 Ent ═══
  { row: 4, col: 0.00,  w: 1.75, label: 'Shift', code: 160 },
  { row: 4, col: 1.75,  w: 1.0,  label: 'Z',     code: 90 },
  { row: 4, col: 2.75,  w: 1.0,  label: 'X',     code: 88 },
  { row: 4, col: 3.75,  w: 1.0,  label: 'C',     code: 67 },
  { row: 4, col: 4.75,  w: 1.0,  label: 'V',     code: 86 },
  { row: 4, col: 5.75,  w: 1.0,  label: 'B',     code: 66 },
  { row: 4, col: 6.75,  w: 1.0,  label: 'N',     code: 78 },
  { row: 4, col: 7.75,  w: 1.0,  label: 'M',     code: 77 },
  { row: 4, col: 8.75,  w: 1.0,  label: ',',     code: 188 },
  { row: 4, col: 9.75,  w: 1.0,  label: '.',     code: 190 },
  { row: 4, col: 10.75, w: 1.0,  label: '/',     code: 191 },
  // 右列
  { row: 4, col: 11.75, w: 3.25, label: 'Shift', code: 161 },
  // 方向键 ↑（导航区正下方）
  { row: 4, col: 16.25, w: 1.0, label: '↑', code: 38 },
  // 小键盘 Row 4
  { row: 4, col: 18.75, w: 1.0, label: '1',   code: 97 },
  { row: 4, col: 19.75, w: 1.0, label: '2',   code: 98 },
  { row: 4, col: 20.75, w: 1.0, label: '3',   code: 99 },
  { row: 4, col: 21.75, w: 1.0, h: 2, label: 'Ent', code: 13 },

  // ═══ Row 5: Ctrl ~ Ctrl ┃ ← ↓ → ┃ 小键盘 0 . Ent ═══
  { row: 5, col: 0.00,  w: 1.25, label: 'Ctrl', code: 162 },
  { row: 5, col: 1.25,  w: 1.25, label: 'Win',  code: 91 },
  { row: 5, col: 2.50,  w: 1.25, label: 'Alt',  code: 164 },
  { row: 5, col: 3.75,  w: 5.50, label: '',     code: 32 },
  { row: 5, col: 9.25,  w: 1.00, label: 'Alt',  code: 165 },
  { row: 5, col: 10.25, w: 1.00, label: 'Fn',   code: 0 },
  { row: 5, col: 11.25, w: 1.00, label: 'Ctrl', code: 163 },
  // 方向键 ←↓→（对齐 ↑ 下方）
  { row: 5, col: 15.25, w: 1.0, label: '←', code: 37 },
  { row: 5, col: 16.25, w: 1.0, label: '↓', code: 40 },
  { row: 5, col: 17.25, w: 1.0, label: '→', code: 39 },
  // 小键盘 Row 5
  { row: 5, col: 18.75, w: 2.0, label: '0',   code: 96 },
  { row: 5, col: 20.75, w: 1.0, label: '.',   code: 110 },
  // Ent 竖键在 Row 4 跨到 Row 5，此处无独立键
]

export const TOTAL_COLS = 23
export const TOTAL_ROWS = 6
