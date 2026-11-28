(() => {
  const THEME_KEY = 'chiri-theme'
  const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)')
  const darkQuery = matchMedia('(prefers-color-scheme: dark)')
  const FAVICON_URL = '/icons/favicon.svg'
  let faviconSvgPromise = null
  let faviconColor = null
  let faviconBlobUrl = null
  let currentPath = location.pathname + location.search

  // Anchor navigation is handled here, so let us own scroll restoration too.
  if ('scrollRestoration' in history) history.scrollRestoration = 'manual'

  function storedTheme() {
    try {
      const value = localStorage.getItem(THEME_KEY)
      return value === 'dark' || value === 'light' ? value : null
    } catch (_) {
      return null
    }
  }

  function effectiveTheme() {
    return storedTheme() || (darkQuery.matches ? 'dark' : 'light')
  }

  function setStoredTheme(theme) {
    try {
      localStorage.setItem(THEME_KEY, theme)
    } catch (_) {}
  }

  // Retint the SVG favicon to match the applied theme. The link is re-queried
  // on every update because view transitions replace the document head.
  function faviconLink() {
    return (
      document.querySelector('link[rel="icon"]') ||
      document.querySelector('link[rel="shortcut icon"]') ||
      document.querySelector('link[rel="apple-touch-icon"]')
    )
  }

  function loadFaviconSvg() {
    if (!faviconSvgPromise) {
      faviconSvgPromise = fetch(FAVICON_URL)
        .then((response) => {
          if (!response.ok) throw new Error(`${response.status} ${response.statusText}`)
          return response.text()
        })
        .catch((error) => {
          console.warn('Failed to load favicon.svg:', error)
          return null
        })
    }
    return faviconSvgPromise
  }

  async function updateFavicon() {
    const link = faviconLink()
    if (!link) return
    const svg = await loadFaviconSvg()
    if (!svg) return

    const color = effectiveTheme() === 'dark' ? '#ddd' : '#111'
    if (faviconColor === color && link.getAttribute('href') === faviconBlobUrl) return

    try {
      const doc = new DOMParser().parseFromString(svg, 'image/svg+xml')
      if (doc.querySelector('parsererror')) throw new Error('invalid SVG')
      doc.querySelectorAll('style').forEach((style) => style.remove())

      const setFill = (node) => {
        if (node.nodeType !== 1) return
        node.setAttribute('fill', color)
        for (const child of node.children) setFill(child)
      }
      setFill(doc.documentElement)

      const serialized = new XMLSerializer().serializeToString(doc)
      const url = URL.createObjectURL(
        new Blob([serialized], { type: 'image/svg+xml' })
      )
      link.setAttribute('href', url)
      if (faviconBlobUrl) URL.revokeObjectURL(faviconBlobUrl)
      faviconBlobUrl = url
      faviconColor = color
    } catch (error) {
      console.warn('Failed to update favicon color:', error)
    }
  }

  function applyTheme(theme) {
    const root = document.documentElement
    root.classList.toggle('dark', theme === 'dark')
    root.classList.toggle('light', theme === 'light')
    root.style.colorScheme = theme

    const toggle = document.getElementById('theme-toggle')
    if (toggle) {
      const label = theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'
      toggle.setAttribute('aria-label', label)
      toggle.setAttribute('title', label)
    }
    updateFavicon()
  }

  function updateGradientMask() {
    const mask = document.querySelector('.gradient-mask')
    if (mask) mask.style.opacity = window.scrollY >= 64 ? '1' : '0'
  }

  // Footer analog clock: 12-hour hands driven by CSS custom properties.
  const clockFormatter = new Intl.DateTimeFormat('en-US', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: true,
  })

  function updateClock() {
    const parts = clockFormatter.formatToParts(new Date())
    const part = (type) => {
      const found = parts.find((item) => item.type === type)
      return found ? found.value : ''
    }
    const hour = part('hour')
    const minute = part('minute')
    const second = part('second')
    const label = `${hour}:${minute}:${second} ${part('dayPeriod')}`.trim()

    document.querySelectorAll('[data-clock]').forEach((clock) => {
      clock.style.setProperty('--now-h', hour)
      clock.style.setProperty('--now-m', minute)
      clock.style.setProperty('--now-s', second)
      clock.setAttribute('aria-label', label)
    })
  }

  applyTheme(effectiveTheme())
  updateGradientMask()
  updateClock()
  setInterval(updateClock, 500)
  addEventListener('scroll', updateGradientMask, { passive: true })

  darkQuery.addEventListener('change', (event) => {
    if (!storedTheme()) applyTheme(event.matches ? 'dark' : 'light')
  })

  document.addEventListener('click', (event) => {
    const target = event.target instanceof Element ? event.target : null
    const toggle = target && target.closest('#theme-toggle')
    if (!toggle) return

    event.preventDefault()
    event.stopPropagation()
    const next = effectiveTheme() === 'dark' ? 'light' : 'dark'
    setStoredTheme(next)
    applyTheme(next)
  })

  // Footnotes open in a small popover instead of jumping to the notes at the
  // bottom. The hidden definitions stay in the DOM as the popover's source.
  let footnotePopover = null
  let activeFootnoteLink = null

  function closeFootnote() {
    if (footnotePopover) footnotePopover.hidden = true
    if (activeFootnoteLink) activeFootnoteLink.setAttribute('aria-expanded', 'false')
    activeFootnoteLink = null
  }

  function showFootnote(link) {
    const href = link.getAttribute('href') || ''
    if (!href.startsWith('#')) return
    const source = document.getElementById(href.substring(1))
    if (!source || !source.classList.contains('footnote-definition')) return

    if (activeFootnoteLink === link && footnotePopover && !footnotePopover.hidden) {
      closeFootnote()
      return
    }
    closeFootnote()

    if (!footnotePopover) {
      footnotePopover = document.createElement('div')
      footnotePopover.className = 'footnote-popover prose'
      footnotePopover.id = 'footnote-popover'
      footnotePopover.setAttribute('role', 'tooltip')
      document.body.appendChild(footnotePopover)
    }

    const content = source.cloneNode(true)
    const label = content.querySelector('.footnote-definition-label')
    if (label) label.remove()
    footnotePopover.replaceChildren(...Array.from(content.childNodes))
    footnotePopover.hidden = false

    const rect = link.getBoundingClientRect()
    const popRect = footnotePopover.getBoundingClientRect()
    const left = Math.min(
      Math.max(8, rect.left + rect.width / 2 - popRect.width / 2),
      Math.max(8, window.innerWidth - popRect.width - 8)
    )
    const below = rect.bottom + 8
    const top =
      below + popRect.height > window.innerHeight - 8
        ? Math.max(8, rect.top - popRect.height - 8)
        : below
    footnotePopover.style.left = `${left}px`
    footnotePopover.style.top = `${top}px`

    link.setAttribute('aria-expanded', 'true')
    link.setAttribute('aria-controls', footnotePopover.id)
    activeFootnoteLink = link
  }

  function initFootnotes() {
    document.querySelectorAll('.footnote-definition').forEach((definition) => {
      definition.hidden = true
    })
    document.querySelectorAll('.footnote-reference a').forEach((link) => {
      link.setAttribute('aria-expanded', 'false')
    })
  }

  document.addEventListener('click', (event) => {
    const target = event.target instanceof Element ? event.target : null
    const link = target && target.closest('.footnote-reference a')
    if (!link) {
      if (
        footnotePopover &&
        !footnotePopover.hidden &&
        (!target || !target.closest('.footnote-popover'))
      ) {
        closeFootnote()
      }
      return
    }
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
      return
    }
    event.preventDefault()
    showFootnote(link)
  })

  addEventListener('keydown', (event) => {
    if (event.key === 'Escape') closeFootnote()
  })
  addEventListener('scroll', closeFootnote, { passive: true })
  addEventListener('resize', closeFootnote)

  // Chiri clears stale hover styles after navigation and touch events.
  function cleanupHover() {
    document.querySelectorAll('ul a:hover').forEach((link) => {
      link.style.opacity = ''
    })
  }

  document.addEventListener('click', () => setTimeout(cleanupHover, 50))
  document.addEventListener('touchstart', cleanupHover, { passive: true })

  // Chiri-style copy button for highlighted code blocks.
  const copyIcon = `
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="14" height="14" fill="currentColor">
      <path d="M0 6.75C0 5.784.784 5 1.75 5h1.5a.75.75 0 0 1 0 1.5h-1.5a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-1.5a.75.75 0 0 1 1.5 0v1.5A1.75 1.75 0 0 1 9.25 16h-7.5A1.75 1.75 0 0 1 0 14.25Z"></path>
      <path d="M5 1.75C5 .784 5.784 0 6.75 0h7.5C15.216 0 16 .784 16 1.75v7.5A1.75 1.75 0 0 1 14.25 11h-7.5A1.75 1.75 0 0 1 5 9.25Zm1.75-.25a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h7.5a.25.25 0 0 0 .25-.25v-7.5a.25.25 0 0 0-.25-.25Z"></path>
    </svg>
  `

  const copiedIcon = `
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16" fill="currentColor">
      <path d="M13.78 4.22a.75.75 0 0 1 0 1.06l-7.25 7.25a.75.75 0 0 1-1.06 0L2.22 9.28a.751.751 0 0 1 .018-1.042.751.751 0 0 1 1.042-.018L6 10.94l6.72-6.72a.75.75 0 0 1 1.06 0Z"></path>
    </svg>
  `

  const copyResetTimers = new WeakMap()

  function setCopyState(button, state) {
    button.dataset.copyState = state
  }

  function showIdleState(button) {
    setCopyState(button, 'idle')
    button.innerHTML = copyIcon
  }

  function showCopiedState(button) {
    window.clearTimeout(copyResetTimers.get(button))

    setCopyState(button, 'copied')
    button.innerHTML = copiedIcon

    const resetTimer = window.setTimeout(() => {
      copyResetTimers.delete(button)
      showIdleState(button)
    }, 1500)

    copyResetTimers.set(button, resetTimer)
  }

  function codeTextFor(button) {
    // The button is a sibling of the `<pre>`, so `closest` alone is not enough.
    let pre = button.closest('.copy-code-block')
    if (!pre) {
      const parent = button.parentElement
      if (parent) pre = parent.querySelector('.copy-code-block')
    }
    if (!pre) return ''

    const code = pre.querySelector('code') || pre
    // The HTML parser already drops the newline right after `<pre>`, so this is
    // exactly the displayed code with no extra transformations.
    return code.textContent || ''
  }

  function initCopyCode() {
    if (document.body.dataset.copyCode === 'disabled') return
    document.body.setAttribute('data-copy-code', 'enabled')

    document.querySelectorAll('.copy-button').forEach((button) => {
      // Avoid initializing twice (important for view-transition swaps).
      if (button.dataset.copyInit) return
      button.dataset.copyInit = '1'

      if (!button.querySelector('svg')) button.innerHTML = copyIcon
      setCopyState(button, 'idle')
    })
  }

  // Delegated so buttons inserted by a page swap keep working.
  document.addEventListener('click', async (event) => {
    const target = event.target instanceof Element ? event.target : null
    const button = target && target.closest('.copy-button')
    if (!button || document.body.dataset.copyCode === 'disabled') return

    event.preventDefault()
    try {
      await navigator.clipboard.writeText(codeTextFor(button))
      showCopiedState(button)
    } catch (error) {
      console.error('Failed to copy code:', error)
      showIdleState(button)
    }
  })

  initCopyCode()

  const toc = {
    container: null,
    links: [],
    titleLink: null,
    headings: [],
    map: new Map(),
    activeLink: null,
    activeId: null,
    scrollFrame: null,
    resizeFrame: null
  }

  function tocHasItems() {
    return toc.links.some((link) => !link.classList.contains('toc-title'))
  }

  function layoutMetrics() {
    const cssWidth = parseFloat(getComputedStyle(document.body).getPropertyValue('--content-width'))
    const width = Number.isFinite(cssWidth) && cssWidth > 0 ? cssWidth : 35
    const margin = (window.innerWidth - width * 16) / 2
    const baseMinSpace = 176
    const minSpace = baseMinSpace + (tocHasItems() ? 52 : 12)
    return { margin, baseMinSpace, minSpace }
  }

  function adjustTocPosition() {
    if (!toc.container) return
    const { margin, baseMinSpace, minSpace } = layoutMetrics()

    if (!tocHasItems() || margin < minSpace) {
      toc.container.style.display = 'none'
      toc.container.classList.remove('fixed-position')
      toc.container.style.left = ''
      return
    }

    toc.container.style.display = 'block'
    toc.container.classList.add('fixed-position')
    toc.container.style.left = `${margin - baseMinSpace - 40}px`
  }

  function adjustBackButton() {
    const button = document.querySelector('.back-button')
    if (!button) return
    const { margin, baseMinSpace, minSpace } = layoutMetrics()

    if (margin >= minSpace) {
      button.classList.add('fixed-position')
      button.style.left = `${margin - baseMinSpace - 40}px`
    } else {
      button.classList.remove('fixed-position')
      button.style.left = ''
    }
  }

  function setActiveToc(id) {
    const link = id && toc.map.has(id) ? toc.map.get(id) : toc.titleLink
    if (toc.activeId === id && toc.activeLink === link) return

    if (toc.activeLink) toc.activeLink.classList.remove('active')
    if (link) link.classList.add('active')
    toc.activeId = id
    toc.activeLink = link
  }

  function refreshToc() {
    if (!toc.container || !toc.headings.length) {
      setActiveToc(null)
      return
    }
    if (window.scrollY < 16 || toc.headings[0].getBoundingClientRect().top > 128) {
      setActiveToc(null)
      return
    }

    for (let i = toc.headings.length - 1; i >= 0; i--) {
      if (toc.headings[i].getBoundingClientRect().top <= 128) {
        setActiveToc(toc.headings[i].id)
        break
      }
    }
  }

  function scheduleTocRefresh() {
    if (toc.scrollFrame) return
    toc.scrollFrame = requestAnimationFrame(() => {
      toc.scrollFrame = null
      refreshToc()
    })
  }

  function scrollToHash(hash, behavior = 'auto') {
    if (!hash || hash === '#') {
      window.scrollTo({ top: 0, behavior })
      return
    }

    let id = hash.substring(1)
    try {
      id = decodeURIComponent(id)
    } catch (_) {}

    const target = id ? document.getElementById(id) : null
    if (!target) {
      window.scrollTo({ top: 0, behavior })
      return
    }

    const top = target.getBoundingClientRect().top + window.scrollY - 96
    window.scrollTo({ top: Math.max(0, top), behavior })
  }

  function handleAnchorClick(event) {
    const target = event.target instanceof Element ? event.target : null
    const link = target && target.closest('a[href^="#"]')
    if (!link) return
    if (
      event.defaultPrevented ||
      event.button !== 0 ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey ||
      (link.target && link.target !== '_self')
    ) {
      return
    }

    const href = link.getAttribute('href') || ''
    if (!href.startsWith('#')) return

    let id = href.substring(1)
    try {
      id = decodeURIComponent(id)
    } catch (_) {}
    if (id && !document.getElementById(id)) return

    event.preventDefault()
    scrollToHash(href, 'smooth')
    history.pushState(null, '', href)
  }

  function initToc() {
    toc.container = document.querySelector('.toc-container')
    toc.links = toc.container ? Array.from(toc.container.querySelectorAll('.toc-link')) : []
    toc.titleLink = toc.container ? toc.container.querySelector('.toc-link.toc-title') : null
    toc.headings = Array.from(document.querySelectorAll('h1, h2, h3')).filter(
      (heading, index, headings) =>
        !(index === 0 && headings[0].tagName === 'H1') && Boolean(heading.id)
    )
    toc.map = new Map()
    toc.activeLink = null
    toc.activeId = null

    for (const link of toc.links) {
      const href = link.getAttribute('href')
      if (href?.startsWith('#')) toc.map.set(href.substring(1), link)
    }

    adjustTocPosition()
    adjustBackButton()
    refreshToc()
  }

  document.addEventListener('click', handleAnchorClick)
  window.addEventListener('scroll', scheduleTocRefresh, { passive: true })
  window.addEventListener('resize', () => {
    if (toc.resizeFrame) cancelAnimationFrame(toc.resizeFrame)
    toc.resizeFrame = requestAnimationFrame(() => {
      toc.resizeFrame = null
      adjustTocPosition()
      adjustBackButton()
      refreshToc()
    })
  })
  initToc()
  initFootnotes()

  function isInternalLink(link, event) {
    if (
      event.defaultPrevented ||
      event.button !== 0 ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey
    ) {
      return false
    }
    if (link.target && link.target !== '_self') return false
    if (link.hasAttribute('download') || link.dataset.noTransition !== undefined) return false

    const url = new URL(link.href, location.href)
    if (url.origin !== location.origin) return false
    if (url.protocol !== 'http:' && url.protocol !== 'https:') return false

    // Let the browser handle same-page anchors.
    if (url.pathname === location.pathname && url.search === location.search && url.hash) {
      return false
    }

    return true
  }

  function swapDocument(doc, url, push) {
    document.documentElement.lang = doc.documentElement.lang || document.documentElement.lang

    document.head.innerHTML = doc.head.innerHTML
    document.body.innerHTML = doc.body.innerHTML
    document.body.className = doc.body.className
    document.body.setAttribute('style', doc.body.getAttribute('style') || '')

    for (const attribute of ['data-centered', 'data-width', 'data-copy-code']) {
      const value = doc.body.getAttribute(attribute)
      if (value) {
        document.body.setAttribute(attribute, value)
      } else {
        document.body.removeAttribute(attribute)
      }
    }

    if (push) history.pushState({}, '', url)
    currentPath = `${url.pathname}${url.search}`
    applyTheme(effectiveTheme())
    if (url.hash && url.hash !== '#') {
      requestAnimationFrame(() => {
        scrollToHash(url.hash)
        updateGradientMask()
      })
    } else {
      window.scrollTo(0, 0)
      updateGradientMask()
    }
    initToc()
    initFootnotes()
    initCopyCode()
    updateClock()
    footnotePopover = null
    activeFootnoteLink = null
  }

  async function navigate(url, push) {
    try {
      const response = await fetch(url, {
        headers: { 'X-Requested-With': 'view-transition' },
      })
      if (!response.ok) {
        location.assign(url)
        return
      }

      const doc = new DOMParser().parseFromString(await response.text(), 'text/html')
      const swap = () => swapDocument(doc, url, push)

      if (document.startViewTransition && !reducedMotion.matches) {
        document.startViewTransition(swap)
      } else {
        swap()
      }
    } catch (_) {
      location.assign(url)
    }
  }

  document.addEventListener('click', (event) => {
    const target = event.target instanceof Element ? event.target : null
    const link = target && target.closest('a[href]')
    if (!link || !isInternalLink(link, event)) return

    const url = new URL(link.href, location.href)
    if (url.href === location.href) return

    event.preventDefault()
    navigate(url, true)
  })

  addEventListener('popstate', () => {
    const url = new URL(location.href)
    if (`${url.pathname}${url.search}` === currentPath) {
      scrollToHash(url.hash)
      updateGradientMask()
      return
    }
    navigate(url, false)
  })
})()
