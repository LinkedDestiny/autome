// Element construction. Every node in this app is built here.
//
// There is no `innerHTML` anywhere in the renderer, and this module is the
// reason there does not need to be. Task titles, milestone text, session log
// lines, skill names and error messages all originate outside the renderer —
// in a repository the user cloned, in a document an agent wrote, in a Rust
// error string. Interpolating any of that into markup would make "an agent
// wrote a <img onerror> into a design document" a renderer exploit, not a
// typo. `text()` and `h()` set `textContent`, so the worst such a string can
// do is look odd.
//
// The one place markup is unavoidable is the SVG sprite, and that is static
// in index.html rather than built from data.

/**
 * Creates an element.
 *
 * `spec` may be `'div'`, `'div.card'`, `'div.card.card--pad'` — the class
 * shorthand is here because the design system is almost entirely classes and
 * spelling out `{ class: '...' }` for every node buried the structure.
 *
 * `attrs` values: `null`/`undefined`/`false` skip the attribute entirely,
 * which is what makes conditional attributes readable at the call site.
 * `onClick` and friends attach listeners. `style` is a *rejected* key: the
 * CSP forbids inline style attributes, so use `el.style.setProperty` on the
 * returned node.
 */
export function h(spec, attrs, children) {
  const [tag, ...classes] = String(spec).split('.');
  const el = document.createElement(tag || 'div');
  if (classes.length) el.className = classes.join(' ');

  if (attrs && typeof attrs === 'object' && !Array.isArray(attrs) && !(attrs instanceof Node)) {
    for (const [key, value] of Object.entries(attrs)) {
      if (value === null || value === undefined || value === false) continue;
      if (key === 'style') {
        throw new Error('h(): inline style attributes are blocked by the CSP; use el.style.setProperty');
      }
      if (key === 'class') {
        el.className = el.className ? `${el.className} ${value}` : String(value);
      } else if (key === 'text') {
        el.textContent = String(value);
      } else if (key === 'dataset') {
        for (const [dk, dv] of Object.entries(value)) {
          if (dv !== null && dv !== undefined) el.dataset[dk] = String(dv);
        }
      } else if (key.startsWith('on') && typeof value === 'function') {
        el.addEventListener(key.slice(2).toLowerCase(), value);
      } else if (value === true) {
        el.setAttribute(key, '');
      } else {
        el.setAttribute(key, String(value));
      }
    }
  } else if (attrs !== undefined && attrs !== null) {
    // Two-argument form: h('div.card', [children]) or h('span', 'text').
    return append(el, attrs);
  }

  return append(el, children);
}

function append(el, children) {
  if (children === null || children === undefined || children === false) return el;
  if (Array.isArray(children)) {
    for (const child of children) append(el, child);
    return el;
  }
  if (children instanceof Node) {
    el.appendChild(children);
    return el;
  }
  el.appendChild(document.createTextNode(String(children)));
  return el;
}

/** A text node. Named so call sites read as data, not markup. */
export function text(value) {
  return document.createTextNode(value === null || value === undefined ? '' : String(value));
}

/** `<svg class="ic"><use href="#i-..."/></svg>` against the static sprite. */
export function icon(name, extra) {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('class', extra ? `ic ${extra}` : 'ic');
  const use = document.createElementNS('http://www.w3.org/2000/svg', 'use');
  use.setAttribute('href', `#i-${name}`);
  svg.appendChild(use);
  return svg;
}

/** Empties a node without `innerHTML = ''`, which would still parse markup. */
export function clear(el) {
  while (el.firstChild) el.removeChild(el.firstChild);
  return el;
}

/**
 * The design system's staggered reveal. The mock wrote `style="--i:3"`; the
 * CSP will not let us, so the custom property is set through CSSOM instead.
 */
export function reveal(el, index) {
  el.classList.add('reveal');
  el.style.setProperty('--i', String(index));
  return el;
}

/** A `.tag` pill. `variant` is the modifier suffix, e.g. `'solid-teal'`. */
export function tag(label, variant, iconName) {
  const el = h('span.tag', variant ? { class: `tag--${variant}` } : null);
  if (iconName) el.appendChild(icon(iconName));
  el.appendChild(text(label));
  return el;
}

/** A `.tag` carrying the spinner the design uses for "in flight". */
export function spinnerTag(label, variant) {
  const el = h('span.tag', variant ? { class: `tag--${variant}` } : null, [h('span.spin')]);
  el.appendChild(text(label));
  return el;
}

/** The dotted progress pill. `ratio` is 0..1; `label` is the right-hand text. */
export function progress(ratio, label, fillVariant) {
  const fill = h('div.progress__fill', fillVariant ? { class: `progress__fill--${fillVariant}` } : null);
  const pct = Math.max(0, Math.min(1, Number.isFinite(ratio) ? ratio : 0)) * 100;
  fill.style.setProperty('width', `${pct}%`);
  return h('div.progress', [
    h('div.progress__track.progress__track--sm', [fill]),
    h('span.progress__info.num.small', { text: label }),
  ]);
}

/** A `<dl class="repo">` from `[[term, description], ...]`. */
export function repoList(rows, extraClass) {
  const dl = h('dl.repo', extraClass ? { class: extraClass } : null);
  for (const [term, description] of rows) {
    dl.appendChild(h('dt', { text: term }));
    dl.appendChild(h('dd', { text: description }));
  }
  return dl;
}

/** A `<dl class="kv">` from `[[term, description], ...]`. */
export function kvList(rows) {
  const dl = h('dl.kv');
  for (const [term, description] of rows) {
    dl.appendChild(h('dt', { text: term }));
    dl.appendChild(description instanceof Node ? h('dd', [description]) : h('dd', { text: description }));
  }
  return dl;
}

/** The card header strip: title on the left, optional node on the right. */
export function cardHead(title, right) {
  const head = h('div.card__head', [h('h3.card__title', { text: title })]);
  if (right) {
    right.classList.add('ml-auto');
    head.appendChild(right);
  }
  return head;
}

/** A section heading: `<div class="sec">` with a title and a muted note. */
export function sectionHead(title, note, right) {
  const sec = h('div.sec.sectionhead', [h('h3', { text: title })]);
  if (note) sec.appendChild(h('span.muted', { text: note }));
  if (right) sec.appendChild(h('div.right', right));
  return sec;
}

/** The "nothing here" block. Says empty, never "unknown". */
export function empty(message) {
  return h('div.empty', { text: message });
}

/**
 * The design makes whole cards clickable. Keyboard users get the same affordance
 * without the card becoming a <button> (which would swallow the buttons inside
 * it). Attach as `onKeyDown` on any element that also has an `onClick`.
 */
export function activateOnKey(event) {
  if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault();
    event.currentTarget.click();
  }
}
