# 前端指南

## 1. Design direction

The interface is a minimal questionnaire tool. Content density is low on student pages and moderate on admin pages. Do not add hero art, decorative gradients, dashboards with charts, animation, nested cards, or marketing copy. Every page must be usable with a keyboard and a narrow phone viewport.

## 2. Design tokens

### Colors

```css
:root {
  --color-bg: #F8FAFC;
  --color-surface: #FFFFFF;
  --color-surface-muted: #F1F5F9;
  --color-border: #CBD5E1;
  --color-border-strong: #94A3B8;
  --color-text: #0F172A;
  --color-text-muted: #475569;
  --color-primary: #2563EB;
  --color-primary-hover: #1D4ED8;
  --color-primary-soft: #DBEAFE;
  --color-success: #15803D;
  --color-success-soft: #DCFCE7;
  --color-warning: #B45309;
  --color-warning-soft: #FEF3C7;
  --color-danger: #B91C1C;
  --color-danger-soft: #FEE2E2;
  --color-focus: #0EA5E9;
}
```

Use `--color-bg` for page background, `--color-surface` for form regions, and `--color-text` for body text. Do not create additional one-off colors except for image previews. Contrast must meet WCAG AA: normal text 4.5:1, large text 3:1, focus indicator clearly visible.

### Typography

- Primary stack: `-apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif`.
- Monospace stack for submission numbers: `ui-monospace, SFMono-Regular, Menlo, Consolas, monospace`.
- Body: 16px, line-height 1.6, weight 400.
- Small helper text: 14px, line-height 1.5.
- Page title: 28px desktop / 24px mobile, weight 700.
- Section title: 20px, weight 600.
- Label: 14px, weight 600.
- Letter spacing: `0` everywhere; never use negative tracking.

### Spacing and shape

Use a 4px base scale: `4, 8, 12, 16, 20, 24, 32, 40, 48, 64px`. Default page padding is 16px mobile and 24px desktop. Form controls use 12px vertical and 14px horizontal padding, `8px` radius, 1px borders. Cards are limited to repeated list items or genuinely framed tools; do not put a card inside another card.

### Layout

- Student content max width: 720px.
- Admin content max width: 1280px.
- Page header height: 56px; keep actions in a single row where possible.
- Form sections use a vertical gap of 24px.
- Admin table uses horizontal scrolling on narrow screens; never shrink text below 13px to force fit.

## 3. Breakpoints

Use CSS media queries at exactly:

- `0-639px`: single-column mobile layout, full-width controls, stacked action buttons.
- `640-1023px`: tablet layout, two-column field groups when each field has at least 280px.
- `1024px+`: desktop layout, admin table and filter row may be inline; student form remains centered at max 720px.

Do not scale font sizes with viewport width. Use `min()`, `max()`, grid tracks, and wrapping to protect content. Test at 320x568, 390x844, 768x1024, and 1440x900.

## 4. Component catalog

There is no shadcn/ui dependency because v1 forbids a Node build chain. Implement a small server-rendered component set in Askama partials with shadcn-like neutral tokens:

- `PageShell`: header, breadcrumb/context line, main, footer.
- `Notice`: neutral, warning, success, error variants.
- `Field`: label, required marker, control, helper, error.
- `TextInput`, `DateInput`, `NumberInput`, `Textarea`, `Select`, `RadioGroup`, `CheckboxGroup`.
- `FileDropzone`: native `<input type=file multiple>` with accepted types and count/size hints.
- `Button`: primary, secondary, danger; icons are optional and only for familiar actions.
- `StatusBadge`: Pending, Approved, Needs Revision, Rejected with text and color.
- `DataTable`: semantic table with caption, sortable-looking static headers, filter row, empty state.
- `Modal`: only for destructive confirmation; normal flows use pages.

Each component must keep labels in the DOM, support `:focus-visible`, and render server-side usable HTML without JavaScript.

Form sections are sibling white surfaces with a 24px gap and clear section legends; their outer grouping stays unframed. Native result/declaration radios use bordered selection blocks with both text and checked controls. Admin counts, filters and tables form separate groups; the record surface and the spaced code/review sidebar align at the top. These refinements use the existing tokens and do not introduce nested display cards.

## 5. Student pages

Home shows the current year and deadline in the first viewport, followed by notices and the access form. Submit uses a progress-like section order without a progress bar: identity, result choice, common fields, category fields, attachments, confirmation. Do not hide required instructions in tooltips. Category sections are toggled with a select; hidden controls are disabled so irrelevant values are not submitted.

Deadline text explicitly includes UTC. Without JavaScript, Update Fields buttons next to result/category choices perform an authenticated, CSRF-checked server refresh; text is preserved and a helper tells students to select files afterward. On script initialization those buttons are both hidden and disabled. The final submit remains fully validated. A refreshed editor stays open.

The edit code on Success uses a monospace block with a copy button and a visible “save this code” warning. The Query page has only two fields and one primary action. Detail pages show status before long content and keep the edit action near the status.

## 6. Admin pages

Admin pages prioritize scanning: a compact top bar, count strip, filters, then the table. Use plain rectangular surfaces, a maximum of 8px radius, and no chart dashboard. The detail page uses a two-column desktop layout (record and review panel) and stacks on mobile. Review controls are always within a POST form with a visible Save action.

## 7. States and copy

- Loading: native submit state; disable only the clicked submit button and change its text to “提交中…”.
- Empty: explain what is empty and provide the next valid action.
- Error: show a short Chinese sentence next to the field and a summary at the top for forms.
- Success: confirm the action and show the next action.
- Status labels: `待审核`, `已通过`, `需补充材料`, `不予认定`.
- Never display stack traces, SQL text, absolute paths, secret values, or raw validation keys.

## 8. Accessibility and interaction

- Every input has a visible label and a stable `id`.
- Error text uses `aria-describedby`; invalid controls set `aria-invalid=true`.
- Use native buttons and links; do not make a `<div>` clickable.
- Keyboard order follows visual order. Focus is never removed after server errors.
- File inputs announce accepted types and limits in visible helper text.
- Color is never the only status signal; every badge includes text.

## 9. CSS implementation rules

Use one stylesheet with tokens at the top, base styles, layout utilities, components, then responsive overrides. Avoid `!important`, deep selectors, magic negative margins, viewport-scaled type, and decorative gradients. Use `box-sizing:border-box`, `max-width:100%`, `overflow-wrap:anywhere` for user content, and stable dimensions for buttons, badges, table cells, and file rows.
