(function () {
  'use strict';

  const ICONS = {
    clipboard: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="5" y="4" width="14" height="17" rx="3"/><path d="M9 4.5V3.8A1.8 1.8 0 0 1 10.8 2h2.4A1.8 1.8 0 0 1 15 3.8v.7"/><path d="M9 9h6M9 13h6M9 17h4"/></svg>',
    image: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="16" rx="3"/><circle cx="8.5" cy="9" r="1.6"/><path d="m4 17 4.5-4.5 3 3 2.5-2.5L20 18"/></svg>',
    settings: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3.1"/><path d="M19.3 14.6a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.09A1.7 1.7 0 0 0 8.8 19.3a1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.7 15a1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.09A1.7 1.7 0 0 0 4.7 8.9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.54h.01A1.7 1.7 0 0 0 10 3V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1.03 1.56 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.3 9v.01A1.7 1.7 0 0 0 21 10h.09a2 2 0 1 1 0 4H21a1.7 1.7 0 0 0-1.7 1.56Z"/></svg>',
    refresh: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M20 7v5h-5"/><path d="M4 17v-5h5"/><path d="M6.1 9a7 7 0 0 1 11.6-2.2L20 9M4 15l2.3 2.2A7 7 0 0 0 17.9 15"/></svg>',
    search: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="6.5"/><path d="m16 16 4.5 4.5"/></svg>',
    close: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6.5 6.5 17.5 17.5"/><path d="M17.5 6.5 6.5 17.5"/></svg>',
    'chevron-left': '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m14.5 6-6 6 6 6"/></svg>',
    'chevron-right': '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m9.5 6 6 6-6 6"/></svg>',
    'chevron-down': '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 9.5 6 6 6-6"/></svg>',
    'chevron-up': '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 14.5 6-6 6 6"/></svg>',
    target: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="7.5"/><circle cx="12" cy="12" r="2.4"/><path d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2"/></svg>',
    database: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><ellipse cx="12" cy="5.5" rx="7.5" ry="3"/><path d="M4.5 5.5v6c0 1.66 3.36 3 7.5 3s7.5-1.34 7.5-3v-6"/><path d="M4.5 11.5v6c0 1.66 3.36 3 7.5 3s7.5-1.34 7.5-3v-6"/></svg>',
    folder: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3.5 7.5A2.5 2.5 0 0 1 6 5h3.2l2 2.3H18a2.5 2.5 0 0 1 2.5 2.5v7A2.5 2.5 0 0 1 18 19.3H6a2.5 2.5 0 0 1-2.5-2.5Z"/></svg>',
    trash: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4.5 7h15M9.5 7V5.4A1.4 1.4 0 0 1 10.9 4h2.2a1.4 1.4 0 0 1 1.4 1.4V7"/><path d="M6.5 7l.8 12a2 2 0 0 0 2 1.9h5.4a2 2 0 0 0 2-1.9l.8-12"/><path d="M10 11v6M14 11v6"/></svg>',
    download: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3.5v11"/><path d="m7.5 10 4.5 4.5L16.5 10"/><path d="M4.5 20.5h15"/></svg>',
    copy: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="8" y="8" width="11.5" height="12.5" rx="2.5"/><path d="M16 8V5.5A2 2 0 0 0 14 3.5H6.5a2 2 0 0 0-2 2V15a2 2 0 0 0 2 2H8"/></svg>',
    pin: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m9 3 6 0-.7 5.1 3 3.4H5.7l3-3.4Z"/><path d="M12 11.5V21"/></svg>',
    'pin-off': '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m4 4 16 16"/><path d="M9 3h6l-.7 5.1 3 3.4h-5.5"/><path d="M12 11.5V21"/></svg>',
    external: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M14 4.5h5.5V10"/><path d="m19 5-7 7"/><path d="M18.5 13.5v4a2 2 0 0 1-2 2h-9a2 2 0 0 1-2-2v-9a2 2 0 0 1 2-2h4"/></svg>',
    check: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m4.5 12.5 5 5 10-11"/></svg>',
    sparkle: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m12 3 1.7 5.3L19 10l-5.3 1.7L12 17l-1.7-5.3L5 10l5.3-1.7Z"/><path d="m18.5 16 .7 2.3 2.3.7-2.3.7-.7 2.3-.7-2.3-2.3-.7 2.3-.7Z"/></svg>',
    file: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M7 3.5h6l4.5 4.6v11.4a1.5 1.5 0 0 1-1.5 1.5H7a1.5 1.5 0 0 1-1.5-1.5V5A1.5 1.5 0 0 1 7 3.5Z"/><path d="M13 3.5V8h4.5"/></svg>',
    text: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M5 6h14M5 10h14M5 14h10M5 18h7"/></svg>',
    alert: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3.8 21 20H3Z"/><path d="M12 9v5"/><path d="M12 17.2h.01"/></svg>',
    calendar: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3.5" y="5" width="17" height="15.5" rx="3"/><path d="M7.5 3v4M16.5 3v4M3.5 9.5h17"/><path d="M8 13h2M14 13h2M8 17h2M14 17h2"/></svg>',
    save: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M5 3.5h11l3.5 3.5v11.5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5.5a2 2 0 0 1 2-2Z"/><path d="M7.5 3.5v5h8v-5"/><path d="M7.5 20.5v-6h9v6"/></svg>',
    'arrow-down': '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M12 4v16"/><path d="m6 14 6 6 6-6"/></svg>',
    info: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="8.5"/><path d="M12 11v5"/><path d="M12 8h.01"/></svg>',
    monitor: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="12.5" rx="2.5"/><path d="M9 20.5h6M12 16.5v4"/></svg>'
  };

  const MONTHS = ['января', 'февраля', 'марта', 'апреля', 'мая', 'июня', 'июля', 'августа', 'сентября', 'октября', 'ноября', 'декабря'];
  const MONTHS_NOM = ['Январь', 'Февраль', 'Март', 'Апрель', 'Май', 'Июнь', 'Июль', 'Август', 'Сентябрь', 'Октябрь', 'Ноябрь', 'Декабрь'];
  const WEEKDAYS = ['воскресенье', 'понедельник', 'вторник', 'среда', 'четверг', 'пятница', 'суббота'];
  const CLIP_LIMIT = 50;
  const SHOT_LIMIT = 36;
  const QUERY_KEY = 'thodclip.search';

  const state = {
    activeTab: 'clips',
    clips: [],
    clipsTotal: 0,
    clipsLoading: false,
    clipsPending: false,
    query: '',
    kind: '',
    pinnedOnly: false,
    date: null,
    shots: [],
    shotsTotal: 0,
    shotsLoading: false,
    shotsPending: false,
    shotsLoaded: false,
    calendarMonth: new Date(new Date().getFullYear(), new Date().getMonth(), 1),
    calendarMap: new Map(),
    calendarData: null,
    calendarLoading: false,
    calendarPending: false,
    settings: null,
    settingsDataDir: '',
    status: null,
    captureEnabled: true,
    expanded: new Set(),
    fullTexts: new Map(),
    lightbox: null,
    importTimer: null,
    lastImportError: '',
    pollTimer: null,
    calendarTimer: null,
    queryTimer: null
  };

  const $ = id => document.getElementById(id);
  const dom = {};

  function cacheDom() {
    [
      'capturePill', 'captureLabel', 'refreshButton', 'settingsButton', 'clipTabCount', 'shotTabCount',
      'searchInput', 'searchClear', 'clipsView', 'shotsView', 'calendarTitle', 'calendarPrev', 'calendarToday',
      'calendarNext', 'calendarGrid', 'calendarSummary', 'storageSize', 'storageClips', 'storageShots',
      'openDataFolder', 'clipsList', 'clipsEmpty', 'loadMoreClips', 'clipsTotalLabel',
      'dayBanner', 'dayBannerText', 'dayBannerClear', 'newItems', 'newItemsRefresh', 'shotsGrid', 'shotsEmpty',
      'loadMoreShots', 'shotsTotalLabel', 'importShots', 'openShotsFolder', 'importShotsEmpty',
      'drawerBackdrop', 'settingsDrawer', 'settingsClose', 'settingsForm', 'setCaptureEnabled', 'setHonorExclusions',
      'setAutoImport', 'setDedupe', 'setMaxClips', 'setMaxText', 'setMaxShots', 'folderList', 'importFromSettings',
      'aboutVersion', 'aboutDataDir', 'aboutStorage', 'settingsOpenFolder', 'settingsImport', 'settingsClearClips',
      'settingsClearShots', 'saveSettings', 'cancelSettings', 'modalBackdrop', 'modalTitle', 'modalText',
      'modalCheckWrap', 'modalCheck', 'modalCheckLabel', 'modalCancel', 'modalConfirm', 'lightbox', 'lightboxTitle',
      'lightboxMeta', 'lightboxStage', 'lightboxImage', 'lightboxLoader', 'lightboxNote', 'lightboxClose',
      'lightboxCopy', 'lightboxSave', 'lightboxReveal', 'lightboxDelete', 'toasts'
    ].forEach(id => { dom[id] = $(id); });
  }

  function icon(name) {
    const span = document.createElement('span');
    span.dataset.icon = name;
    span.innerHTML = ICONS[name] || ICONS.info;
    return span;
  }

  function hydrateIcons(root = document) {
    root.querySelectorAll('[data-icon]').forEach(node => {
      if (!node.firstElementChild) node.innerHTML = ICONS[node.dataset.icon] || ICONS.info;
    });
  }

  function el(tag, className, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  }

  function rpc(method, params) {
    return window.thodHub.call('worker.' + method, params ?? null);
  }

  function bridge(method, params) {
    return window.thodHub.call(method, params ?? null);
  }

  function pad(value) {
    return String(value).padStart(2, '0');
  }

  function localKey(date = new Date()) {
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  }

  function monthKey(date) {
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}`;
  }

  function parseKey(key) {
    const [year, month, day] = key.split('-').map(Number);
    return new Date(year, month - 1, day || 1);
  }

  function dateTime(iso) {
    const date = new Date(iso);
    return Number.isNaN(date.getTime()) ? '—' : date.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' });
  }

  function dayLong(key) {
    const date = parseKey(key);
    return `${date.getDate()} ${MONTHS[date.getMonth()]} ${date.getFullYear()}`;
  }

  function dayTitle(key) {
    const today = localKey();
    const yesterday = localKey(new Date(Date.now() - 86400000));
    if (key === today) return 'Сегодня';
    if (key === yesterday) return 'Вчера';
    return dayLong(key);
  }

  function shortDate(iso) {
    const date = new Date(iso);
    if (Number.isNaN(date.getTime())) return '—';
    return `${date.getDate()} ${MONTHS[date.getMonth()]}`;
  }

  function plural(count, forms) {
    const value = Math.abs(count) % 100;
    const last = value % 10;
    if (value > 10 && value < 20) return forms[2];
    if (last > 1 && last < 5) return forms[1];
    if (last === 1) return forms[0];
    return forms[2];
  }

  function formatNumber(value) {
    return Number(value || 0).toLocaleString('ru-RU');
  }

  function formatBytes(bytes) {
    const value = Number(bytes || 0);
    if (value < 1024) return `${value} Б`;
    const units = ['КБ', 'МБ', 'ГБ', 'ТБ'];
    let size = value / 1024;
    let index = 0;
    while (size >= 1024 && index < units.length - 1) {
      size /= 1024;
      index++;
    }
    return `${size >= 10 ? Math.round(size) : size.toFixed(1).replace('.', ',')} ${units[index]}`;
  }

  function showToast(message, options = {}) {
    if (!message) return;
    const toast = el('div', 'toast' + (options.error ? ' is-error' : ''));
    toast.append(icon(options.error ? 'alert' : (options.icon || 'check')));
    toast.append(el('span', '', String(message)));
    dom.toasts.append(toast);
    while (dom.toasts.children.length > 4) dom.toasts.firstElementChild.remove();
    const timeout = options.timeout ?? (options.error ? 6500 : 3800);
    setTimeout(() => {
      if (!toast.isConnected) return;
      toast.classList.add('is-leaving');
      setTimeout(() => toast.remove(), 220);
    }, timeout);
  }

  function errorText(error) {
    const message = error && error.message ? error.message : String(error);
    return message.replace(/^Error:\s*/i, '');
  }

  function baseName(path) {
    const parts = String(path || '').split(/[\\/]/).filter(Boolean);
    return parts.length ? parts[parts.length - 1] : '';
  }

  function looksTemporary(name) {
    const value = String(name || '');
    if (/^\{?[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\}?(\.[a-z0-9]{1,5})?$/i.test(value)) return true;
    if (/^[0-9a-f]{16,}\.[a-z0-9]{1,5}$/i.test(value)) return true;
    return /^(tmp|temp|~)[-_]/i.test(value);
  }

  function setBodyOverlay() {
    const open = !dom.settingsDrawer.hidden || !dom.lightbox.hidden || !dom.modalBackdrop.hidden;
    document.body.classList.toggle('has-overlay', open);
  }

  async function init() {
    cacheDom();
    hydrateIcons();
    wireEvents();

    if (!window.thodHub) {
      showToast('Нет связи с ThodHUB. Откройте ThodClip через каталог.', { error: true, timeout: 12000 });
      return;
    }

    restoreQuery();
    await Promise.all([loadSettings(), loadStatus()]);
    await loadCalendar(true);
    await loadClips(0);
    state.pollTimer = setInterval(pollStatus, 12000);
  }

  function restoreQuery() {
    try {
      const saved = sessionStorage.getItem(QUERY_KEY);
      if (saved) {
        state.query = saved;
        dom.searchInput.value = saved;
        dom.searchClear.hidden = false;
      }
    } catch { }
  }

  function wireEvents() {
    document.querySelectorAll('.tab').forEach(tab => {
      tab.addEventListener('click', () => switchTab(tab.dataset.tab));
    });

    dom.searchInput.addEventListener('input', () => {
      clearTimeout(state.queryTimer);
      dom.searchClear.hidden = !dom.searchInput.value;
      state.queryTimer = setTimeout(() => {
        state.query = dom.searchInput.value.trim();
        try { sessionStorage.setItem(QUERY_KEY, state.query); } catch { }
        reloadActive(0);
      }, 280);
    });

    dom.searchClear.addEventListener('click', () => {
      dom.searchInput.value = '';
      dom.searchClear.hidden = true;
      state.query = '';
      try { sessionStorage.removeItem(QUERY_KEY); } catch { }
      reloadActive(0);
      dom.searchInput.focus();
    });

    dom.capturePill.addEventListener('click', toggleCapture);
    dom.refreshButton.addEventListener('click', () => refreshAll(true));
    dom.settingsButton.addEventListener('click', openSettings);
    dom.settingsClose.addEventListener('click', closeSettings);
    dom.cancelSettings.addEventListener('click', closeSettings);
    dom.drawerBackdrop.addEventListener('click', closeSettings);
    dom.settingsForm.addEventListener('submit', saveSettings);

    dom.calendarPrev.addEventListener('click', () => shiftCalendar(-1));
    dom.calendarNext.addEventListener('click', () => shiftCalendar(1));
    dom.calendarToday.addEventListener('click', () => {
      const today = new Date();
      state.calendarMonth = new Date(today.getFullYear(), today.getMonth(), 1);
      loadCalendar();
    });

    document.querySelectorAll('.chip').forEach(chip => {
      chip.addEventListener('click', () => {
        document.querySelectorAll('.chip').forEach(other => other.classList.toggle('is-active', other === chip));
        const filter = chip.dataset.filter;
        state.kind = filter === 'text' || filter === 'files' || filter === 'image' ? filter : '';
        state.pinnedOnly = filter === 'pinned';
        loadClips(0);
      });
    });

    dom.openDataFolder.addEventListener('click', revealDataFolder);
    dom.settingsOpenFolder.addEventListener('click', revealDataFolder);
    dom.settingsClearClips.addEventListener('click', clearClips);
    dom.dayBannerClear.addEventListener('click', () => selectDay(null));
    dom.newItemsRefresh.addEventListener('click', () => {
      dom.newItems.hidden = true;
      loadClips(0);
      loadCalendar();
      loadStatus();
    });
    dom.loadMoreClips.addEventListener('click', () => loadClips(state.clips.length));

    dom.importShots.addEventListener('click', importScreenshots);
    dom.importShotsEmpty.addEventListener('click', importScreenshots);
    dom.settingsImport.addEventListener('click', importScreenshots);
    dom.importFromSettings.addEventListener('click', importScreenshots);
    dom.openShotsFolder.addEventListener('click', () => rpc('storage.reveal').catch(err => showToast(errorText(err), { error: true })));
    dom.settingsClearShots.addEventListener('click', clearScreenshots);
    dom.loadMoreShots.addEventListener('click', () => loadShots(state.shots.length));

    dom.lightboxClose.addEventListener('click', closeLightbox);
    dom.lightbox.addEventListener('click', event => {
      if (event.target === dom.lightbox) closeLightbox();
    });
    dom.lightboxCopy.addEventListener('click', copyLightbox);
    dom.lightboxSave.addEventListener('click', saveLightbox);
    dom.lightboxReveal.addEventListener('click', revealLightbox);
    dom.lightboxDelete.addEventListener('click', deleteLightbox);

    dom.modalConfirm.addEventListener('click', () => finishModal(true));
    dom.modalCancel.addEventListener('click', () => finishModal(false));
    dom.modalBackdrop.addEventListener('click', event => {
      if (event.target === dom.modalBackdrop) finishModal(false);
    });

    document.addEventListener('keydown', event => {
      if (event.key === 'Enter' && !dom.modalBackdrop.hidden) {
        const target = event.target;
        if (target && target.tagName === 'BUTTON' && target !== dom.modalConfirm) return;
        event.preventDefault();
        finishModal(true);
        return;
      }
      if (event.key !== 'Escape') return;
      if (!dom.modalBackdrop.hidden) { event.preventDefault(); finishModal(false); return; }
      if (!dom.lightbox.hidden) { event.preventDefault(); closeLightbox(); return; }
      if (!dom.settingsDrawer.hidden) { event.preventDefault(); closeSettings(); }
    });

    document.addEventListener('visibilitychange', () => {
      if (!document.hidden) pollStatus();
    });
  }

  async function loadStatus() {
    try {
      const status = await rpc('status');
      applyStatus(status);
    } catch (error) {
      showToast('Не удалось получить состояние: ' + errorText(error), { error: true });
    }
  }

  function applyStatus(status) {
    const previous = state.status;
    state.status = status;
    state.captureEnabled = !!status.captureEnabled;
    updateCapturePill(status);
    updateCounts(status);

    if (dom.aboutVersion) dom.aboutVersion.textContent = status.version || '—';
    if (dom.aboutDataDir) {
      dom.aboutDataDir.textContent = status.dataDir || '—';
      dom.aboutDataDir.title = status.dataDir || '';
    }
    if (dom.aboutStorage) dom.aboutStorage.textContent = formatBytes(status.storageBytes);
    if (dom.storageSize) dom.storageSize.textContent = formatBytes(status.storageBytes);
    if (dom.storageClips) dom.storageClips.textContent = formatNumber(status.clipCount);
    if (dom.storageShots) dom.storageShots.textContent = formatNumber(status.shotCount);
    if (!dom.settingsDrawer.hidden) renderFolders(status.folders || []);

    if (status.import && status.import.running) watchImport();
    if (status.import && !status.import.running && status.import.error && status.import.error !== state.lastImportError) {
      state.lastImportError = status.import.error;
      showToast('Импорт скриншотов: ' + status.import.error, { error: true });
    }

    if (previous && status.clipCount > previous.clipCount && state.activeTab === 'clips' && dom.lightbox.hidden) {
      dom.newItems.hidden = false;
      scheduleCalendarRefresh();
    } else if (previous && status.clipCount !== previous.clipCount) {
      scheduleCalendarRefresh();
    }

    if (previous && status.shotCount > previous.shotCount && state.activeTab === 'shots' && dom.lightbox.hidden) {
      loadShots(0);
    }

    if (dom.settingsDrawer.hidden) fillSettingsForm(status);
  }

  function updateCapturePill(status) {
    const enabled = !!status.captureEnabled;
    const active = status.captureActive !== false;
    dom.capturePill.classList.toggle('is-off', !enabled || !active);
    dom.captureLabel.textContent = !enabled ? 'Запись выключена' : active ? 'Запись включена' : 'Запись не активна';
    dom.capturePill.title = enabled ? 'Выключить запись буфера обмена' : 'Включить запись буфера обмена';
  }

  function updateCounts(status) {
    if (!status) return;
    dom.clipTabCount.textContent = formatNumber(status.clipCount);
    dom.shotTabCount.textContent = formatNumber(status.shotCount);
  }

  async function toggleCapture() {
    const next = !state.captureEnabled;
    dom.capturePill.disabled = true;
    try {
      await rpc('capture.set', { enabled: next });
      state.captureEnabled = next;
      if (state.status) state.status.captureEnabled = next;
      updateCapturePill(state.status || { captureEnabled: next, captureActive: true });
      if (!dom.settingsDrawer.hidden) dom.setCaptureEnabled.checked = next;
      showToast(next ? 'Запись буфера включена' : 'Запись буфера выключена', { icon: next ? 'check' : 'info' });
    } catch (error) {
      showToast(errorText(error), { error: true });
    } finally {
      dom.capturePill.disabled = false;
    }
  }

  async function refreshAll(announce) {
    dom.refreshButton.classList.add('is-spinning');
    try {
      await loadStatus();
      await loadCalendar();
      if (state.activeTab === 'clips') await loadClips(0);
      else await loadShots(0);
      if (announce) showToast('Обновлено');
    } finally {
      dom.refreshButton.classList.remove('is-spinning');
    }
  }

  function reloadActive(offset) {
    if (state.activeTab === 'clips') loadClips(offset);
    else loadShots(offset);
  }

  function switchTab(tab) {
    if (!tab || tab === state.activeTab) return;
    state.activeTab = tab;
    document.querySelectorAll('.tab').forEach(node => node.classList.toggle('is-active', node.dataset.tab === tab));
    dom.clipsView.classList.toggle('is-active', tab === 'clips');
    dom.shotsView.classList.toggle('is-active', tab === 'shots');
    dom.searchInput.placeholder = tab === 'clips' ? 'Поиск по истории' : 'Поиск по снимкам';
    if (tab === 'clips') {
      loadClips(0);
    } else {
      loadShots(0);
    }
  }

  // ------------------------------------------------------------------ clips

  async function loadClips(offset) {
    if (state.clipsLoading) {
      if (offset === 0) state.clipsPending = true;
      return;
    }
    state.clipsLoading = true;
    dom.refreshButton.classList.add('is-spinning');
    try {
      const page = await rpc('clips.list', {
        query: state.query || null,
        kind: state.kind || null,
        pinned: state.pinnedOnly,
        date: state.date,
        offset,
        limit: CLIP_LIMIT
      });
      if (offset === 0) {
        state.clips = page.items;
      } else {
        const seen = new Set(state.clips.map(item => item.id));
        state.clips = state.clips.concat(page.items.filter(item => !seen.has(item.id)));
      }
      state.clipsTotal = page.total;
      if (state.status) state.status.clipCount = page.total;
      dom.newItems.hidden = true;
      renderClips();
      updateDayBanner();
    } catch (error) {
      showToast('Не удалось загрузить историю: ' + errorText(error), { error: true });
    } finally {
      state.clipsLoading = false;
      dom.refreshButton.classList.remove('is-spinning');
      if (state.clipsPending) {
        state.clipsPending = false;
        loadClips(0);
      }
    }
  }

  function renderClips() {
    dom.clipsList.replaceChildren();
    const pinned = state.clips.filter(item => item.pinned);
    const rest = state.clips.filter(item => !item.pinned);

    if (pinned.length) {
      dom.clipsList.append(buildGroup('Закреплённые', pinned.length, pinned, true));
    }

    const groups = new Map();
    rest.forEach(item => {
      const key = localKey(new Date(item.createdAt));
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key).push(item);
    });
    Array.from(groups.keys()).sort().reverse().forEach(key => {
      dom.clipsList.append(buildGroup(dayTitle(key), groups.get(key).length, groups.get(key), false, key));
    });

    const empty = state.clips.length === 0;
    dom.clipsEmpty.hidden = !empty;
    if (empty) {
      const filtered = state.query || state.kind || state.pinnedOnly || state.date;
      dom.clipsEmpty.querySelector('h3').textContent = filtered ? 'Ничего не найдено' : 'Пока пусто';
      dom.clipsEmpty.querySelector('p').textContent = filtered
        ? 'Попробуйте изменить запрос, фильтр или выбранный день.'
        : 'Скопируйте что-нибудь — текст, файлы или картинку. ThodClip сохранит запись сюда и не потеряет её после перезапуска.';
    }

    const more = state.clips.length < state.clipsTotal;
    dom.loadMoreClips.hidden = !more;
    dom.clipsTotalLabel.textContent = state.clipsTotal
      ? `Показано ${formatNumber(state.clips.length)} из ${formatNumber(state.clipsTotal)}`
      : '';
    dom.clipTabCount.textContent = formatNumber(state.clipsTotal);
  }

  function buildGroup(title, count, items, pinned, key) {
    const group = el('section', 'day-group');
    const heading = el('div', 'day-heading');
    const label = el('strong', '', title);
    const total = el('span', '', `${formatNumber(count)} ${plural(count, ['запись', 'записи', 'записей'])}`);
    heading.append(label, total);
    group.append(heading);
    items.forEach(item => group.append(buildClipCard(item)));
    return group;
  }

  function buildClipCard(item) {
    const card = el('article', 'clip-card' + (item.pinned ? ' is-pinned' : ''));
    card.dataset.kind = item.kind;
    card.dataset.id = String(item.id);

    const type = el('div', 'clip-type');
    type.append(icon(item.kind === 'image' ? 'image' : item.kind === 'files' ? 'file' : 'text'));

    const main = el('div', 'clip-main');
    const meta = el('div', 'clip-meta');
    const source = el('span', 'source', item.source || (item.kind === 'image' ? 'Изображение' : item.kind === 'files' ? 'Файлы' : 'Текст'));
    source.title = source.textContent;
    meta.append(source, el('time', '', dateTime(item.createdAt)));
    if (item.repeatCount > 1) meta.append(el('span', 'badge repeat', '×' + formatNumber(item.repeatCount)));
    if (item.pinned) meta.append(el('span', 'badge pin', 'закреплено'));
    if (item.truncated) meta.append(el('span', 'badge', 'обрезано'));
    const remove = el('button', 'clip-delete');
    remove.type = 'button';
    remove.title = 'Удалить запись';
    remove.setAttribute('aria-label', 'Удалить запись');
    remove.append(icon('close'));
    remove.addEventListener('click', event => {
      event.stopPropagation();
      removeClip(item);
    });
    meta.append(remove);
    main.append(meta);

    if (item.kind === 'image') {
      const name = (item.files || []).map(baseName).find(value => value && !looksTemporary(value)) || '';
      const text = el('p', 'clip-text', `Изображение ${formatNumber(item.width)} × ${formatNumber(item.height)} · ${formatBytes(item.bytes)}`);
      if (name) {
        text.textContent = `${name} · ${formatNumber(item.width)} × ${formatNumber(item.height)} · ${formatBytes(item.bytes)}`;
        text.title = item.files[0];
      }
      main.append(text);
      if (item.thumb) {
        const thumb = el('button', 'clip-thumb');
        thumb.type = 'button';
        thumb.title = 'Открыть изображение';
        const img = document.createElement('img');
        img.src = item.thumb;
        img.alt = `Изображение ${item.width} на ${item.height}`;
        img.loading = 'lazy';
        thumb.append(img);
        thumb.addEventListener('click', () => openLightbox('clip', item.id));
        main.append(thumb);
      }
    } else if (item.kind === 'files') {
      const list = el('ul', 'clip-files');
      (item.files || []).slice(0, 4).forEach(path => {
        const line = el('li', 'file-line');
        line.append(icon('file'));
        const name = String(path).split(/[\\/]/).filter(Boolean).pop() || path;
        const span = el('span', '', name);
        span.title = path;
        line.append(span);
        list.append(line);
      });
      if ((item.files || []).length > 4) list.append(el('li', 'file-more', `и ещё ${formatNumber(item.files.length - 4)}`));
      main.append(list);
    } else {
      const textNode = el('p', 'clip-text');
      const canExpand = item.textClipped || (item.text || '').length > 700 || ((item.text || '').match(/\n/g) || []).length > 7;
      const cached = state.fullTexts.get(item.id);
      const expanded = state.expanded.has(item.id);
      textNode.textContent = expanded && cached ? cached : (item.text || '');
      if (expanded) textNode.classList.add('is-expanded');
      main.append(textNode);

      if (canExpand || item.textClipped) {
        const expand = el('button', 'clip-expand');
        expand.type = 'button';
        setExpandContent(expand, expanded, expanded && cached ? cached.length : (item.textClipped ? (item.bytes || 0) : (item.text || '').length), item.textClipped);
        expand.addEventListener('click', async () => {
          if (state.expanded.has(item.id)) {
            state.expanded.delete(item.id);
            textNode.textContent = state.fullTexts.get(item.id) || item.text || '';
            textNode.classList.remove('is-expanded');
            setExpandContent(expand, false, (item.text || '').length, item.textClipped);
            return;
          }
          let full = state.fullTexts.get(item.id) || (item.textClipped ? '' : item.text);
          if (!full && item.textClipped) {
            expand.disabled = true;
            expand.replaceChildren(icon('refresh'), el('span', '', 'Загрузка…'));
            try {
              const detail = await rpc('clips.get', { id: item.id });
              full = detail.item.text || '';
              state.fullTexts.set(item.id, full);
            } catch (error) {
              showToast('Не удалось раскрыть текст: ' + errorText(error), { error: true });
              setExpandContent(expand, false, (item.text || '').length, true);
              return;
            } finally {
              expand.disabled = false;
            }
          }
          state.expanded.add(item.id);
          textNode.textContent = full || item.text || '';
          textNode.classList.add('is-expanded');
          setExpandContent(expand, true, (full || item.text || '').length, false);
        });
        main.append(expand);
      }
    }

    const actions = el('div', 'clip-actions');
    actions.append(copyButton(item));
    if (item.kind === 'files') {
      const paths = actionButton('file', 'Пути', async () => {
        await copyClip(item, 'paths');
      }, 'Пути');
      actions.append(paths);
    }
    const pin = actionButton(item.pinned ? 'pin-off' : 'pin', item.pinned ? 'Снять' : 'Закрепить', async () => {
      try {
        await rpc('clips.pin', { id: item.id, pinned: !item.pinned });
        state.fullTexts.delete(item.id);
        state.expanded.delete(item.id);
        await Promise.all([loadClips(0), loadCalendar()]);
      } catch (error) {
        showToast(errorText(error), { error: true });
      }
    }, item.pinned ? 'Снять с закреплённых' : 'Закрепить');
    actions.append(pin);

    card.append(type, main, actions);
    return card;
  }

  function setExpandContent(button, expanded, length, clipped) {
    button.replaceChildren(
      icon(expanded ? 'chevron-up' : 'chevron-down'),
      el('span', '', expanded ? 'Свернуть' : 'Раскрыть'),
      el('span', 'chars', expanded ? `${formatNumber(length)} симв.` : clipped ? 'полный текст' : `${formatNumber(length)} симв.`)
    );
  }

  function actionButton(iconName, label, handler, title, extraClass) {
    const button = el('button', 'action-button' + (extraClass ? ' ' + extraClass : ''));
    button.type = 'button';
    button.title = title || label;
    if (label) button.append(icon(iconName), el('span', '', label));
    else button.append(icon(iconName));
    button.addEventListener('click', async event => {
      event.stopPropagation();
      button.disabled = true;
      try { await handler(); }
      finally { button.disabled = false; }
    });
    return button;
  }

  function copyButton(item) {
    const button = el('button', 'action-button primary');
    button.type = 'button';
    button.title = 'Скопировать';
    button.append(icon('copy'), el('span', '', 'Копировать'));
    button.addEventListener('click', async () => {
      button.disabled = true;
      try {
        const ok = await copyClip(item);
        const card = button.closest('.clip-card');
        if (ok && card) {
          card.classList.add('is-copied');
          setTimeout(() => card.classList.remove('is-copied'), 900);
        }
      } finally {
        button.disabled = false;
      }
    });
    return button;
  }

  async function copyClip(item, mode = 'default') {
    try {
      await rpc('clips.copy', { id: item.id, mode });
      showToast(mode === 'paths' ? 'Пути скопированы' : item.kind === 'files' ? 'Файлы скопированы' : 'Скопировано в буфер');
      return true;
    } catch (error) {
      const message = errorText(error);
      showToast(message, { error: true });
      return false;
    }
  }

  async function removeClip(item) {
    const result = await confirmDialog({
      title: 'Удалить запись?',
      text: item.kind === 'text'
        ? 'Текст будет удалён из истории ThodClip без возможности восстановления.'
        : item.kind === 'files'
          ? 'Запись со списком файлов будет удалена. Сами файлы останутся на диске.'
          : 'Изображение будет удалено из истории и с диска внутри папки данных.',
      confirmText: 'Удалить',
      danger: true
    });
    if (!result.ok) return;
    try {
      await rpc('clips.delete', { ids: [item.id] });
      state.fullTexts.delete(item.id);
      state.expanded.delete(item.id);
      showToast('Запись удалена');
      await Promise.all([loadClips(0), loadCalendar()]);
    } catch (error) {
      showToast(errorText(error), { error: true });
    }
  }

  async function clearClips() {
    const result = await confirmDialog({
      title: 'Очистить историю буфера?',
      text: 'Все записи ThodClip будут удалены. Системный буфер обмена и история Windows (Win+V) не изменятся.',
      confirmText: 'Очистить',
      danger: true,
      check: { label: 'Оставить закреплённые записи', checked: true }
    });
    if (!result.ok) return;
    try {
      const response = await rpc('clips.clear', { keepPinned: result.checked });
      state.fullTexts.clear();
      state.expanded.clear();
      showToast(`Удалено: ${formatNumber(response.deleted)}`);
      await Promise.all([loadClips(0), loadCalendar(), loadStatus()]);
    } catch (error) {
      showToast(errorText(error), { error: true });
    }
  }

  function updateDayBanner() {
    if (!state.date) {
      dom.dayBanner.hidden = true;
      return;
    }
    dom.dayBanner.hidden = false;
    const count = state.clipsTotal;
    dom.dayBannerText.textContent = `${dayLong(state.date)} · ${formatNumber(count)} ${plural(count, ['запись', 'записи', 'записей'])}`;
  }

  async function selectDay(key) {
    if (!key || key === state.date) {
      state.date = null;
    } else {
      state.date = key;
      const date = parseKey(key);
      state.calendarMonth = new Date(date.getFullYear(), date.getMonth(), 1);
      renderCalendar();
    }
    updateDayBanner();
    await loadClips(0);
    if (state.date) {
      const target = document.querySelector('.content');
      if (target) target.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
  }

  // -------------------------------------------------------------- calendar

  async function loadCalendar(autoJump) {
    if (state.calendarLoading) {
      state.calendarPending = true;
      return;
    }
    state.calendarLoading = true;
    try {
      const month = monthKey(state.calendarMonth);
      const data = await rpc('clips.calendar', { month });
      if (autoJump && data.total === 0 && data.lastDate && monthKey(parseKey(data.lastDate)) !== month) {
        state.calendarMonth = new Date(parseKey(data.lastDate).getFullYear(), parseKey(data.lastDate).getMonth(), 1);
        state.calendarLoading = false;
        return loadCalendar(false);
      }
      state.calendarData = data;
      state.calendarMap = new Map((data.days || []).map(entry => [entry.date, entry.count]));
      renderCalendar();
    } catch (error) {
      showToast('Не удалось загрузить календарь: ' + errorText(error), { error: true });
    } finally {
      state.calendarLoading = false;
      if (state.calendarPending) {
        state.calendarPending = false;
        loadCalendar();
      }
    }
  }

  function renderCalendar() {
    const year = state.calendarMonth.getFullYear();
    const month = state.calendarMonth.getMonth();
    dom.calendarTitle.textContent = `${MONTHS_NOM[month]} ${year}`;

    const first = new Date(year, month, 1);
    const offset = (first.getDay() + 6) % 7;
    const lastDay = new Date(year, month + 1, 0).getDate();
    const counts = Array.from(state.calendarMap.values());
    const max = counts.length ? Math.max(...counts) : 0;
    const today = localKey();
    const grid = dom.calendarGrid;
    grid.replaceChildren();

    for (let i = 0; i < offset; i++) grid.append(el('span', 'day is-empty'));

    for (let day = 1; day <= lastDay; day++) {
      const key = `${year}-${pad(month + 1)}-${pad(day)}`;
      const count = state.calendarMap.get(key) || 0;
      const button = el('button', 'day');
      button.type = 'button';
      button.dataset.date = key;
      if (count > 0) button.classList.add('heat-' + heatLevel(count, max));
      if (key === today) button.classList.add('is-today');
      if (key === state.date) button.classList.add('is-selected');
      button.append(el('span', '', String(day)));
      if (count > 0) button.append(el('span', 'day-count', count > 99 ? '99+' : String(count)));
      const label = `${dayLong(key)}: ${count} ${plural(count, ['запись', 'записи', 'записей'])}`;
      button.title = label;
      button.setAttribute('aria-label', label);
      button.addEventListener('click', () => selectDay(key));
      grid.append(button);
    }

    const total = state.calendarData ? state.calendarData.total : 0;
    dom.calendarSummary.textContent = total
      ? `${formatNumber(total)} ${plural(total, ['копия', 'копии', 'копий'])} за месяц`
      : 'Нет записей';
  }

  function heatLevel(count, max) {
    if (count <= 0) return 0;
    if (max <= 1) return 1;
    const ratio = count / max;
    if (ratio > 0.66) return 4;
    if (ratio > 0.4) return 3;
    if (ratio > 0.15) return 2;
    return 1;
  }

  function shiftCalendar(delta) {
    state.calendarMonth = new Date(state.calendarMonth.getFullYear(), state.calendarMonth.getMonth() + delta, 1);
    loadCalendar();
  }

  function scheduleCalendarRefresh() {
    if (state.calendarTimer) return;
    state.calendarTimer = setTimeout(() => {
      state.calendarTimer = null;
      loadCalendar();
    }, 500);
  }

  // ----------------------------------------------------------------- shots

  async function loadShots(offset) {
    if (state.shotsLoading) {
      if (offset === 0) state.shotsPending = true;
      return;
    }
    state.shotsLoading = true;
    dom.refreshButton.classList.add('is-spinning');
    try {
      const page = await rpc('shots.list', {
        query: state.query || null,
        offset,
        limit: SHOT_LIMIT
      });
      if (offset === 0) {
        state.shots = page.items;
      } else {
        const seen = new Set(state.shots.map(item => item.id));
        state.shots = state.shots.concat(page.items.filter(item => !seen.has(item.id)));
      }
      state.shotsTotal = page.total;
      state.shotsLoaded = true;
      if (state.status) state.status.shotCount = page.total;
      renderShots();
    } catch (error) {
      showToast('Не удалось загрузить снимки: ' + errorText(error), { error: true });
    } finally {
      state.shotsLoading = false;
      dom.refreshButton.classList.remove('is-spinning');
      if (state.shotsPending) {
        state.shotsPending = false;
        loadShots(0);
      }
    }
  }

  function renderShots() {
    dom.shotsGrid.replaceChildren();
    state.shots.forEach(item => dom.shotsGrid.append(buildShotCard(item)));
    dom.shotsEmpty.hidden = state.shots.length > 0;
    if (!state.shots.length) {
      const filtered = state.query;
      dom.shotsEmpty.querySelector('h3').textContent = filtered ? 'Ничего не найдено' : 'Снимков пока нет';
      dom.shotsEmpty.querySelector('p').textContent = filtered
        ? 'Попробуйте изменить поисковый запрос.'
        : 'Сделайте скриншот через Snipping Tool или сохраните его в папку «Изображения». ThodClip найдёт и заархивирует файл.';
      dom.importShotsEmpty.hidden = !!filtered;
    } else {
      dom.importShotsEmpty.hidden = false;
    }
    dom.loadMoreShots.hidden = state.shots.length >= state.shotsTotal;
    dom.shotsTotalLabel.textContent = state.shotsTotal
      ? `Показано ${formatNumber(state.shots.length)} из ${formatNumber(state.shotsTotal)}`
      : '';
    dom.shotTabCount.textContent = formatNumber(state.shotsTotal);
  }

  function buildShotCard(item) {
    const card = el('article', 'shot-card');
    const thumb = el('button', 'shot-thumb');
    thumb.type = 'button';
    thumb.title = 'Открыть снимок';
    if (item.thumb) {
      const img = document.createElement('img');
      img.src = item.thumb;
      img.alt = item.name || 'Скриншот';
      img.loading = 'lazy';
      thumb.append(img);
    } else {
      thumb.append(icon('image'));
    }
    const open = el('span', 'shot-open');
    open.append(icon('external'));
    thumb.append(open);
    thumb.addEventListener('click', () => openLightbox('shot', item.id));

    const info = el('div', 'shot-info');
    info.append(el('p', 'shot-name', item.name || 'Снимок'));
    info.append(el('p', 'shot-meta', `${formatNumber(item.width)}×${formatNumber(item.height)} · ${formatBytes(item.bytes)} · ${shortDate(item.createdAt)}`));

    const actions = el('div', 'shot-actions');
    actions.append(actionButton('copy', '', () => copyShot(item), 'Копировать изображение'));
    actions.append(actionButton('save', '', () => saveShot(item), 'Сохранить как…'));
    actions.append(actionButton('folder', '', async () => {
      try { await rpc('shots.reveal', { id: item.id }); }
      catch (error) { showToast(errorText(error), { error: true }); }
    }, 'Показать в папке'));

    const remove = el('button', 'shot-delete');
    remove.type = 'button';
    remove.title = 'Удалить снимок';
    remove.setAttribute('aria-label', 'Удалить снимок');
    remove.append(icon('close'));
    remove.addEventListener('click', event => {
      event.stopPropagation();
      removeShot(item);
    });

    card.append(thumb, info, actions, remove);
    return card;
  }

  async function copyShot(item) {
    try {
      await rpc('shots.copy', { id: item.id });
      showToast('Изображение скопировано');
    } catch (error) {
      showToast(errorText(error), { error: true });
    }
  }

  async function saveShot(item) {
    try {
      const detail = await rpc('shots.get', { id: item.id });
      await saveDataUrl(detail.dataUrl, item.name || `thodclip-${item.id}`, detail.mime, detail.preview);
    } catch (error) {
      showToast(errorText(error), { error: true });
    }
  }

  async function removeShot(item) {
    const result = await confirmDialog({
      title: 'Удалить снимок?',
      text: 'Архивная копия будет удалена из папки данных ThodClip. Оригинал, если он ещё существует, не затрагивается.',
      confirmText: 'Удалить',
      danger: true
    });
    if (!result.ok) return;
    try {
      await rpc('shots.delete', { ids: [item.id] });
      showToast('Снимок удалён');
      await loadShots(0);
    } catch (error) {
      showToast(errorText(error), { error: true });
    }
  }

  async function importScreenshots() {
    try {
      const response = await rpc('shots.import');
      showToast(response.message || 'Импорт запущен', { icon: 'download' });
      watchImport();
    } catch (error) {
      showToast(errorText(error), { error: true });
    }
  }

  function watchImport() {
    if (state.importTimer) return;
    let ticks = 0;
    state.importTimer = setInterval(async () => {
      ticks++;
      try {
        const status = await rpc('status');
        applyStatus(status);
        if (!status.import.running || ticks >= 90) {
          clearInterval(state.importTimer);
          state.importTimer = null;
          if (status.import.imported > 0 || status.import.failed > 0) {
            const parts = [`добавлено ${formatNumber(status.import.imported)}`];
            if (status.import.failed > 0) parts.push(`ошибок ${formatNumber(status.import.failed)}`);
            showToast('Импорт скриншотов: ' + parts.join(', '), { icon: 'image' });
          }
          if (state.activeTab === 'shots') await loadShots(0);
        }
      } catch {
        clearInterval(state.importTimer);
        state.importTimer = null;
      }
    }, 650);
  }

  async function clearScreenshots() {
    const result = await confirmDialog({
      title: 'Очистить архив скриншотов?',
      text: 'Все архивные копии снимков будут удалены из папки данных ThodClip. Оригиналы в ваших папках останутся на месте.',
      confirmText: 'Очистить',
      danger: true
    });
    if (!result.ok) return;
    try {
      const response = await rpc('shots.clear');
      showToast(`Удалено снимков: ${formatNumber(response.deleted)}`);
      state.shots = [];
      state.shotsTotal = 0;
      await loadShots(0);
    } catch (error) {
      showToast(errorText(error), { error: true });
    }
  }

  // -------------------------------------------------------------- settings

  async function loadSettings() {
    try {
      const response = await rpc('settings.get');
      state.settings = response.settings;
      state.settingsDataDir = response.dataDir;
      fillSettingsForm(state.status);
    } catch (error) {
      showToast('Не удалось загрузить настройки: ' + errorText(error), { error: true });
    }
  }

  function fillSettingsForm(status) {
    if (!state.settings || !dom.setCaptureEnabled) return;
    const settings = state.settings;
    dom.setCaptureEnabled.checked = !!settings.captureEnabled;
    dom.setHonorExclusions.checked = !!settings.honorClipboardExclusions;
    dom.setAutoImport.checked = !!settings.autoImportScreenshots;
    dom.setDedupe.value = settings.dedupeSeconds;
    dom.setMaxClips.value = settings.maxClips;
    dom.setMaxText.value = Math.round(settings.maxTextBytes / 1024);
    dom.setMaxShots.value = Math.round(settings.maxScreenshotBytes / 1024 / 1024);
    if (status) renderFolders(status.folders || []);
    if (status && dom.aboutVersion) {
      dom.aboutVersion.textContent = status.version || '—';
      dom.aboutDataDir.textContent = status.dataDir || '—';
      dom.aboutDataDir.title = status.dataDir || '';
      dom.aboutStorage.textContent = formatBytes(status.storageBytes);
    }
  }

  function renderFolders(folders) {
    dom.folderList.replaceChildren();
    if (!folders.length) {
      dom.folderList.append(el('div', 'folder-item', 'Папки не найдены — импорт недоступен.'));
      return;
    }
    folders.slice(0, 8).forEach(folder => {
      const row = el('div', 'folder-item');
      row.append(icon('folder'));
      const code = el('code', '', folder);
      code.title = folder;
      row.append(code);
      dom.folderList.append(row);
    });
    if (folders.length > 8) dom.folderList.append(el('div', 'folder-item', `и ещё ${folders.length - 8}…`));
  }

  function openSettings() {
    fillSettingsForm(state.status);
    dom.settingsDrawer.hidden = false;
    dom.drawerBackdrop.hidden = false;
    requestAnimationFrame(() => {
      dom.settingsDrawer.classList.add('is-open');
      dom.settingsDrawer.setAttribute('aria-hidden', 'false');
    });
    setBodyOverlay();
    setTimeout(() => dom.settingsClose.focus(), 180);
  }

  function closeSettings() {
    dom.settingsDrawer.classList.remove('is-open');
    dom.settingsDrawer.setAttribute('aria-hidden', 'true');
    dom.drawerBackdrop.hidden = true;
    setTimeout(() => { dom.settingsDrawer.hidden = true; setBodyOverlay(); }, 280);
  }

  async function saveSettings(event) {
    event.preventDefault();
    const payload = {
      captureEnabled: dom.setCaptureEnabled.checked,
      honorClipboardExclusions: dom.setHonorExclusions.checked,
      autoImportScreenshots: dom.setAutoImport.checked,
      dedupeSeconds: Number.parseInt(dom.setDedupe.value, 10) || 0,
      maxClips: Number.parseInt(dom.setMaxClips.value, 10) || 0,
      maxTextBytes: (Number.parseInt(dom.setMaxText.value, 10) || 1024) * 1024,
      maxScreenshotBytes: (Number.parseInt(dom.setMaxShots.value, 10) || 0) * 1024 * 1024
    };
    dom.saveSettings.disabled = true;
    const original = dom.saveSettings.innerHTML;
    dom.saveSettings.replaceChildren(icon('refresh'), el('span', '', 'Сохранение…'));
    try {
      const response = await rpc('settings.set', payload);
      state.settings = response.settings;
      state.captureEnabled = response.settings.captureEnabled;
      fillSettingsForm(state.status);
      closeSettings();
      showToast('Настройки сохранены');
      await Promise.all([loadStatus(), loadCalendar(), loadClips(0)]);
    } catch (error) {
      showToast('Не удалось сохранить: ' + errorText(error), { error: true });
    } finally {
      dom.saveSettings.disabled = false;
      dom.saveSettings.innerHTML = original;
      hydrateIcons(dom.saveSettings);
    }
  }

  function revealDataFolder() {
    rpc('storage.reveal').catch(error => showToast(errorText(error), { error: true }));
  }

  // --------------------------------------------------------------- lightbox

  async function openLightbox(kind, id) {
    state.lightbox = { kind, id, item: null, dataUrl: '', preview: false };
    dom.lightbox.hidden = false;
    dom.lightboxLoader.hidden = false;
    dom.lightboxImage.removeAttribute('src');
    dom.lightboxTitle.textContent = 'Изображение';
    dom.lightboxMeta.textContent = '';
    dom.lightboxNote.hidden = true;
    dom.lightboxSave.disabled = false;
    dom.lightboxReveal.hidden = false;
    setBodyOverlay();

    try {
      const detail = kind === 'shot'
        ? await rpc('shots.get', { id })
        : await rpc('clips.get', { id });
      const item = detail.item;
      state.lightbox.item = item;
      state.lightbox.dataUrl = detail.dataUrl;
      state.lightbox.preview = !!detail.preview;
      dom.lightboxImage.src = detail.dataUrl;
      dom.lightboxImage.alt = item.name || (item.kind === 'image' ? 'Изображение из буфера' : 'Снимок');
      dom.lightboxLoader.hidden = true;
      dom.lightboxTitle.textContent = kind === 'shot'
        ? (item.name || 'Снимок')
        : `Изображение ${formatNumber(item.width)} × ${formatNumber(item.height)}`;
      const bits = [`${formatNumber(item.width)}×${formatNumber(item.height)}`, formatBytes(item.bytes)];
      if (item.source) bits.push(item.source);
      bits.push(shortDate(item.createdAt));
      dom.lightboxMeta.textContent = bits.join(' · ');
      dom.lightboxNote.hidden = !detail.preview;
      dom.lightboxSave.disabled = !!detail.preview;
      dom.lightboxSave.title = detail.preview ? 'Оригинал больше 5,5 МБ — откройте папку данных' : 'Сохранить как…';
    } catch (error) {
      showToast('Не удалось открыть изображение: ' + errorText(error), { error: true });
      closeLightbox();
    }
  }

  function closeLightbox() {
    dom.lightbox.hidden = true;
    dom.lightboxImage.removeAttribute('src');
    state.lightbox = null;
    setBodyOverlay();
  }

  async function copyLightbox() {
    if (!state.lightbox) return;
    const { kind, item } = state.lightbox;
    try {
      await rpc(kind === 'shot' ? 'shots.copy' : 'clips.copy', { id: item.id });
      showToast('Скопировано в буфер');
    } catch (error) {
      showToast(errorText(error), { error: true });
    }
  }

  async function saveLightbox() {
    if (!state.lightbox || !state.lightbox.dataUrl) return;
    const { item, dataUrl, preview, kind } = state.lightbox;
    if (preview) {
      showToast('Оригинал больше 5,5 МБ — откройте его через кнопку «В папке».', { error: true });
      return;
    }
    const mime = dataUrl.slice(5, dataUrl.indexOf(';'));
    const name = kind === 'shot' ? (item.name || `thodclip-${item.id}`) : `clip-${item.id}`;
    try {
      await saveDataUrl(dataUrl, name, mime, false);
    } catch (error) {
      showToast(errorText(error), { error: true });
    }
  }

  async function saveDataUrl(dataUrl, name, mime, preview) {
    if (!dataUrl) throw new Error('Изображение недоступно.');
    if (preview) throw new Error('Доступен только уменьшенный просмотр. Откройте оригинал в папке данных.');
    const base64 = dataUrl.slice(dataUrl.indexOf(',') + 1);
    if (base64.length * 0.75 > 8 * 1024 * 1024) throw new Error('Файл больше 8 МБ — сохраните его через папку данных.');
    const fileName = buildFileName(name, mime);
    const result = await bridge('files.export', { name: fileName, base64 });
    if (!result.canceled) showToast('Сохранено: ' + (result.filePath || fileName), { icon: 'save' });
  }

  function buildFileName(name, mime) {
    const extension = mime === 'image/jpeg' ? '.jpg'
      : mime === 'image/gif' ? '.gif'
        : mime === 'image/bmp' ? '.bmp'
          : mime === 'image/webp' ? '.webp'
            : mime === 'image/tiff' ? '.tiff'
              : '.png';
    let base = String(name || 'thodclip').replace(/\.(png|jpe?g|gif|bmp|webp|tiff?)$/i, '');
    base = base.replace(/[<>:"/\\|?*\u0000-\u001f]/g, '_').replace(/[. ]+$/, '').slice(0, 120) || 'thodclip';
    return base + extension;
  }

  async function revealLightbox() {
    if (!state.lightbox) return;
    const { kind, item } = state.lightbox;
    try {
      await rpc(kind === 'shot' ? 'shots.reveal' : 'clips.reveal', { id: item.id });
    } catch (error) {
      showToast(errorText(error), { error: true });
    }
  }

  async function deleteLightbox() {
    if (!state.lightbox) return;
    const { kind, item } = state.lightbox;
    const result = await confirmDialog({
      title: 'Удалить изображение?',
      text: kind === 'shot'
        ? 'Архивная копия будет удалена. Оригинал в исходной папке не затрагивается.'
        : 'Изображение будет удалено из истории ThodClip.',
      confirmText: 'Удалить',
      danger: true
    });
    if (!result.ok) return;
    try {
      if (kind === 'shot') {
        await rpc('shots.delete', { ids: [item.id] });
        closeLightbox();
        await loadShots(0);
      } else {
        await rpc('clips.delete', { ids: [item.id] });
        closeLightbox();
        await Promise.all([loadClips(0), loadCalendar()]);
      }
      showToast('Изображение удалено');
    } catch (error) {
      showToast(errorText(error), { error: true });
    }
  }

  // ----------------------------------------------------------------- modal

  let modalResolver = null;

  function confirmDialog({ title, text, confirmText = 'Подтвердить', danger = false, check = null }) {
    if (modalResolver) {
      modalResolver({ ok: false, checked: false });
      modalResolver = null;
    }
    dom.modalTitle.textContent = title;
    dom.modalText.textContent = text;
    dom.modalConfirm.textContent = confirmText;
    dom.modalConfirm.className = danger ? 'danger-button' : 'primary-button';
    dom.modalCheckWrap.hidden = !check;
    dom.modalCheck.checked = check ? !!check.checked : false;
    dom.modalCheckLabel.textContent = check ? check.label : '';
    dom.modalBackdrop.hidden = false;
    setBodyOverlay();
    setTimeout(() => dom.modalConfirm.focus(), 30);
    return new Promise(resolve => { modalResolver = resolve; });
  }

  function finishModal(ok) {
    if (!modalResolver) return;
    const resolve = modalResolver;
    modalResolver = null;
    const checked = dom.modalCheck.checked;
    dom.modalBackdrop.hidden = true;
    setBodyOverlay();
    resolve({ ok, checked });
  }

  // ------------------------------------------------------------------ poll

  async function pollStatus() {
    if (document.hidden) return;
    try {
      const status = await rpc('status');
      applyStatus(status);
    } catch { }
  }

  init();
})();
