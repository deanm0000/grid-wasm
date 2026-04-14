import { test, expect } from '@playwright/test';

async function getLayoutEntries(page: any) {
  return page.evaluate(() =>
    JSON.parse((window as any).__gridDebug.get_layout_debug_info())
  );
}

// ─── Test 1: Print all DRAW and LAYOUT logs to confirm they match ─────────────

test('draw and layout coordinates match — print all logs', async ({ page }) => {
  const logs: string[] = [];
  page.on('console', msg => {
    const t = msg.text();
    if (t.startsWith('[DRAW') || t.startsWith('[LAYOUT') || t.startsWith('[HIT')
        || t.startsWith('[LINES') || t.startsWith('[RECOMPUTE') || t.startsWith('[MOUSEMOVE')) {
      logs.push(t);
    }
  });

  await page.goto('/');
  await page.waitForFunction(() => document.querySelector('canvas') !== null);
  await page.waitForTimeout(2000);

  const canvasRect = await page.evaluate(() => {
    const c = document.querySelector('canvas')!;
    return c.getBoundingClientRect();
  });

  // ── Test at translate_x=0 (no scroll) ──
  let entries = await getLayoutEntries(page);
  const leafY = entries[0]?.leaf_y ?? 30;
  const leafH = entries[0]?.leaf_h ?? 30;
  const headerCssY = canvasRect.top + leafY + leafH / 2;

  // Sweep across the full width at no-scroll
  for (let cssX = canvasRect.left; cssX < canvasRect.left + 1000; cssX += 2) {
    await page.mouse.move(cssX, headerCssY);
  }
  await page.waitForTimeout(200);

  // ── Now scroll right by 300px and repeat ──
  await page.evaluate(() => {
    const g = (window as any).__gridDebug;
    g.set_translate(-300, 0);
    g.render();
  });
  await page.waitForTimeout(300);

  // Re-fetch layout AFTER scroll — this now has the correct scrolled positions
  entries = await getLayoutEntries(page);
  const leafYScrolled = entries[0]?.leaf_y ?? 30;
  const leafHScrolled = entries[0]?.leaf_h ?? 30;
  const headerCssYScrolled = canvasRect.top + leafYScrolled + leafHScrolled / 2;

  // Sweep across the full width while scrolled
  for (let cssX = canvasRect.left; cssX < canvasRect.left + 1000; cssX += 2) {
    await page.mouse.move(cssX, headerCssYScrolled);
  }

  // Wait briefly for any async logging
  await page.waitForTimeout(300);

  console.log('\n========== DRAW vs LAYOUT LOGS ==========');
  for (const l of logs.filter(l => l.startsWith('[RECOMPUTE')).slice(0,3)) console.log(l);
  for (const l of logs.filter(l => l.startsWith('[LAYOUT')).slice(0,8)) console.log(l);
  console.log('--- LINES (walk_columns during draw_grid_lines) ---');
  // Print first occurrence of each LINES col log
  const seenLines = new Set<string>();
  for (const l of logs.filter(l => l.startsWith('[LINES'))) {
    if (l.startsWith('[LINES]')) { console.log(l); continue; }
    const key = l.match(/\[LINES col=\d+\]/)?.[0] ?? l;
    if (!seenLines.has(key)) { seenLines.add(key); console.log(l); }
  }
  console.log('--- DRAW (col_layout.entries during header draw) ---');
  const seenDraw = new Set<string>();
  for (const l of logs.filter(l => l.startsWith('[DRAW'))) {
    const key = l.match(/\[DRAW col=\d+\]/)?.[0] ?? l;
    if (!seenDraw.has(key)) { seenDraw.add(key); console.log(l); }
  }
  console.log('--- MOUSEMOVE near border ---');
  for (const l of logs.filter(l => l.startsWith('[MOUSEMOVE')).slice(0, 5)) console.log(l);
  console.log('--- HIT logs ---');
  for (const l of logs.filter(l => l.startsWith('[HIT')).slice(0, 10)) console.log(l);
  console.log('==========================================\n');

  // Core assertion: for every DRAW log, the draw_x must equal the LAYOUT draw_x for same col
  const layoutByCol: Record<number, { draw_x: number; right: number }> = {};
  for (const l of logs.filter(l => l.startsWith('[LAYOUT col='))) {
    const col = +(l.match(/\[LAYOUT col=(\d+)\]/)?.[1] ?? -1);
    const draw_x = +(l.match(/draw_x=([-\d.]+)/)?.[1] ?? -1);
    const right = +(l.match(/right=([-\d.]+)/)?.[1] ?? -1);
    // Only keep first occurrence (unscrolled, translate_x=0)
    if (col >= 0 && !(col in layoutByCol)) layoutByCol[col] = { draw_x, right };
  }

  const drawByCol: Record<number, { draw_x: number; right: number }> = {};
  for (const l of logs.filter(l => l.startsWith('[DRAW col='))) {
    const col = +(l.match(/\[DRAW col=(\d+)\]/)?.[1] ?? -1);
    const draw_x = +(l.match(/draw_x=([\d.]+)/)?.[1] ?? -1);
    const right = +(l.match(/right=([\d.]+)/)?.[1] ?? -1);
    if (col >= 0 && !drawByCol[col]) drawByCol[col] = { draw_x, right };
  }

  // Also parse LINES logs: [LINES col=N] raw_draw_x=X col_x(line)=Y
  // The line is drawn at col_x which is the LEFT edge of col N (= right edge of col N-1)
  const linesByCol: Record<number, { raw_draw_x: number; line_x: number }> = {};
  for (const l of logs.filter(l => l.startsWith('[LINES col'))) {
    const col = +(l.match(/col=(\d+)/)?.[1] ?? -1);
    const raw_draw_x = +(l.match(/raw_draw_x=([\d.]+)/)?.[1] ?? -1);
    const line_x = +(l.match(/col_x\(line\)=([\d.]+)/)?.[1] ?? -1);
    if (col >= 0 && !linesByCol[col]) linesByCol[col] = { raw_draw_x, line_x };
  }

  // For each col N, the LINES line_x should equal LAYOUT right_border_x of col N-1
  // (because the line at col N's left edge IS the right border of col N-1)
  console.log('\n--- LINES vs LAYOUT border comparison ---');
  for (const [colStr, lineVals] of Object.entries(linesByCol)) {
    const col = +colStr;
    const prevCol = col - 1;
    const layoutRight = layoutByCol[prevCol]?.right;
    if (layoutRight !== undefined) {
      const diff = lineVals.line_x - layoutRight;
      console.log(`  border between col[${prevCol}] and col[${col}]: LINES draws at ${lineVals.line_x}, LAYOUT right=${layoutRight}, diff=${diff.toFixed(1)}`);
    }
  }

  for (const [colStr, layoutVals] of Object.entries(layoutByCol)) {
    const col = +colStr;
    const drawVals = drawByCol[col];
    if (!drawVals) continue;
    expect(drawVals.draw_x, `col[${col}] DRAW draw_x matches LAYOUT draw_x`)
      .toBeCloseTo(layoutVals.draw_x, 0);
    expect(drawVals.right, `col[${col}] DRAW right matches LAYOUT right`)
      .toBeCloseTo(layoutVals.right, 0);
  }
});

// ─── Test 2: Static geometry invariants ───────────────────────────────────────

test('column hit areas are non-overlapping, covering, and symmetric (static)', async ({ page }) => {
  await page.goto('/');
  await page.waitForFunction(() => document.querySelector('canvas') !== null);
  await page.waitForTimeout(2000);

  const entries = await getLayoutEntries(page);
  expect(entries.length).toBeGreaterThan(0);

  for (const e of entries) {
    const col = `col[${e.col}]`;
    const m = e.menu_btn;
    const s = e.sort_triangles;
    const r = e.resize;

    expect(m.hit_x_max, `${col} menu.hit_x_max <= sort.hit_x_min`)
      .toBeLessThanOrEqual(s.hit_x_min + 0.5);
    expect(s.hit_x_max, `${col} sort.hit_x_max <= resize.hit_x_min`)
      .toBeLessThanOrEqual(r.hit_x_min + 0.5);

    const menuLeftDist  = m.draw_x_center - m.hit_x_min;
    const menuRightDist = m.hit_x_max     - m.draw_x_center;
    expect(menuLeftDist,  `${col} menu hit x-symmetric`).toBeCloseTo(menuRightDist, 0);

    const sortLeftDist  = s.draw_x_center - s.hit_x_min;
    const sortRightDist = s.hit_x_max     - s.draw_x_center;
    expect(sortLeftDist,  `${col} sort hit x-symmetric`).toBeCloseTo(sortRightDist, 0);

    const resizeLeftDist  = r.draw_x_center - r.hit_x_min;
    const resizeRightDist = r.hit_x_max     - r.draw_x_center;
    expect(resizeLeftDist, `${col} resize hit x-symmetric`).toBeCloseTo(resizeRightDist, 0);

    expect(s.down.hit_y_min, `${col} sort ▼.hit_y_min == ▲.hit_y_max`).toBeCloseTo(s.up.hit_y_max, 0);

    expect(s.up.hit_y_min,   `${col} ▲ hit top <= draw top`).toBeLessThanOrEqual(s.up.draw_y_min + 0.5);
    expect(s.up.hit_y_max,   `${col} ▲ hit bottom >= draw bottom`).toBeGreaterThanOrEqual(s.up.draw_y_max - 0.5);
    expect(s.down.hit_y_min, `${col} ▼ hit top <= draw top`).toBeLessThanOrEqual(s.down.draw_y_min + 0.5);
    expect(s.down.hit_y_max, `${col} ▼ hit bottom >= draw bottom`).toBeGreaterThanOrEqual(s.down.draw_y_max - 0.5);

    expect(r.draw_x_center, `${col} resize center == right_border_x`).toBeCloseTo(e.right_border_x, 0);
  }
});

// ─── Test 3: Live sweep — resize zone straddles border ────────────────────────

test('resize hit zone straddles column border in both sweep directions', async ({ page }) => {
  const resizeLogs: string[] = [];
  page.on('console', msg => {
    if (msg.text().startsWith('[HIT_RESIZE')) resizeLogs.push(msg.text());
  });

  await page.goto('/');
  await page.waitForFunction(() => document.querySelector('canvas') !== null);
  await page.waitForTimeout(2000);

  const entries = await getLayoutEntries(page);
  expect(entries.length).toBeGreaterThan(0);

  const sweepResults: Array<{
    col: number;
    border: number;
    is_resizable: boolean;
    ltr: { first: number | null; last: number | null };
    rtl: { first: number | null; last: number | null };
  }> = await page.evaluate((entries: any[]) => {
    const grid = (window as any).__gridDebug;

    function sweep(fromX: number, toX: number, y: number, step: number) {
      let first: number | null = null;
      let last: number | null = null;
      const dir = fromX <= toX ? 1 : -1;
      for (let x = fromX; dir > 0 ? x <= toX : x >= toX; x = +(x + dir * step).toFixed(1)) {
        const cursor = grid.on_mouse_move(x, y);
        if (cursor === 'col-resize') {
          if (first === null) first = x;
          last = x;
        }
      }
      return { first, last };
    }

    return entries.map((e: any) => ({
      col: e.col,
      border: e.right_border_x,
      is_resizable: e.is_resizable ?? true,
      ltr: sweep(e.right_border_x - 40, e.right_border_x + 40, e.leaf_y + e.leaf_h / 2, 0.5),
      rtl: sweep(e.right_border_x + 40, e.right_border_x - 40, e.leaf_y + e.leaf_h / 2, 0.5),
    }));
  }, entries);

  console.log('\n========== SWEEP RESULTS ==========');
  for (const r of sweepResults) {
    console.log(`col[${r.col}] border=${r.border} resizable=${r.is_resizable} LTR=[${r.ltr.first},${r.ltr.last}] RTL=[${r.rtl.last},${r.rtl.first}]`);
  }
  console.log('HIT_RESIZE events:', resizeLogs.slice(0, 10));
  console.log('===================================\n');

  for (const r of sweepResults) {
    const col = `col[${r.col}] border=${r.border}`;

    if (!r.is_resizable) {
      expect(r.ltr.first, `${col} non-resizable: no LTR hit`).toBeNull();
      expect(r.rtl.first, `${col} non-resizable: no RTL hit`).toBeNull();
      continue;
    }

    expect(r.ltr.first, `${col} LTR: resize zone found`).not.toBeNull();
    expect(r.rtl.first, `${col} RTL: resize zone found`).not.toBeNull();

    expect(r.ltr.first!, `${col} LTR first <= border`).toBeLessThanOrEqual(r.border + 0.5);
    expect(r.ltr.last!,  `${col} LTR last >= border`).toBeGreaterThanOrEqual(r.border - 0.5);
    expect(r.rtl.first!, `${col} RTL first >= border`).toBeGreaterThanOrEqual(r.border - 0.5);
    expect(r.rtl.last!,  `${col} RTL last <= border`).toBeLessThanOrEqual(r.border + 0.5);

    expect(r.ltr.first!, `${col} LTR/RTL agree left edge`).toBeCloseTo(r.rtl.last!, 0);
    expect(r.ltr.last!,  `${col} LTR/RTL agree right edge`).toBeCloseTo(r.rtl.first!, 0);

    const ltrMid = (r.ltr.first! + r.ltr.last!) / 2;
    const rtlMid = (r.rtl.first! + r.rtl.last!) / 2;
    expect(ltrMid, `${col} LTR midpoint ≈ border`).toBeCloseTo(r.border, 0);
    expect(rtlMid, `${col} RTL midpoint ≈ border`).toBeCloseTo(r.border, 0);
  }
});
