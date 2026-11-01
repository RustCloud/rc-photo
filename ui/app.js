// The Photos page: a timeline of photos and videos with filters and a viewer.
// It runs in the panel's sandboxed <iframe>. JSON requests go through
// rustcloud.fetch; the index, thumbnails and originals come by links the
// plugin hands out, straight from the server, cached by the browser.
(async () => {
  const TEXTS = {
    en: {
      title: 'Photos',
      search: 'Search by name, folder, camera',
      items: (n) => `${n.toLocaleString('en')} ${n === 1 ? 'item' : 'items'}`,
      processing: (n) => `Processing: ${n.toLocaleString('en')} left`,
      failed: (n) => `${n} could not be processed`,
      filters: 'Filters',
      resetAll: 'Clear all',
      type: 'Type',
      all: 'All',
      photos: 'Photos',
      videos: 'Videos',
      onlyFavorites: 'Favorites only',
      withLocation: 'With location',
      period: 'Period',
      from: 'From',
      to: 'To',
      years: 'Years',
      cameras: 'Cameras',
      lenses: 'Lenses',
      folders: 'Folders',
      formats: 'Formats',
      orientation: 'Orientation',
      landscape: 'Landscape',
      portrait: 'Portrait',
      square: 'Square',
      panorama: 'Panorama',
      more: (n) => `${n} more`,
      less: 'Show less',
      newest: 'Newest first',
      oldest: 'Oldest first',
      addFolder: 'Add a folder',
      folderName: 'Photos',
      chooseTitle: 'Show where your photos are',
      chooseHint:
        'The gallery shows photos and videos from folders of your files. They stay where they are.',
      chooseButton: 'Choose a folder',
      denied: 'The gallery needs a folder to show something.',
      removeHint: 'To remove a folder, take the access back in Settings → Apps with access, then Rebuild.',
      rebuild: 'Rebuild',
      rebuildTitle: 'Rebuild the gallery?',
      rebuildText:
        'The gallery walks its folders again: it finds what was missed and drops what it may no longer read. Thumbnails already made stay.',
      rebuildStarted: 'Rebuilding: the gallery walks its folders.',
      delete: 'Delete (Del)',
      deleteTitle: (name) => `Delete “${name}”?`,
      deleteText: 'The file goes to the trash of your files.',
      deleteButton: 'Delete',
      needWriteTitle: 'The gallery may only read this folder',
      needWriteText: (folder) =>
        `To delete files from “${folder}”, let the gallery change it. In the next window choose this folder again.`,
      needWriteUploadText: (folder) =>
        `To upload into “${folder}”, let the gallery change it. In the next window choose this folder again.`,
      allow: 'Continue',
      cancel: 'Cancel',
      upload: 'Upload',
      select: 'Select',
      selected: (n) => `${n.toLocaleString('en')} selected`,
      clearSelection: 'Clear selection (Esc)',
      selectDay: 'Select the day',
      selectAll: 'Select all (Ctrl+A)',
      deleteManyTitle: (n) => `Delete ${n} ${n === 1 ? 'file' : 'files'}?`,
      deleteManyText: 'The files go to the trash of your files.',
      downloadingMany: (i, n) => `Downloading ${i} of ${n}…`,
      deleteFailed: (n) => `Could not delete: ${n}`,
      people: 'People',
      facesOffText: 'The server can find faces in your photos and group them by person. Faces stay in your own data of the plugin.',
      facesOn: 'Find people',
      facesAskTitle: 'Find people in photos?',
      facesAskText:
        'The server looks at every photo with a neural network and keeps a description of each face in your data of the plugin. Nobody else sees it; you can turn it off and delete it at any time.',
      facesDownloading: 'The server is downloading the neural networks…',
      facesNotAllowed: 'The administrator has not yet allowed the plugin neural networks (use.onnx).',
      facesNoEngine: 'This server has no engine for neural networks.',
      facesFailed: 'The neural networks could not be started.',
      facesLooking: (n) => `Looking for faces: ${n} left`,
      facesNone: 'No people yet: a person appears once their face is in a few photos.',
      nameN: (n) => `Name ${n}`,
      attach: 'Attach',
      attachTitle: 'Attach to',
      attachWhole: (n) => `All faces of this person (${plural(n, `${n} photo`, `${n} photos`, `${n} photos`)})`,
      notThem: 'Not this person',
      avatarTitle: 'Face to show',
      newPersonHint: 'A name makes a new person of this face; or attach it to someone.',
      showFaces: 'Faces',
      showHidden: (n) => `Hidden: ${n}`,
      hideHidden: 'Hide them again',
      facesOff: 'Turn off',
      facesForget: 'Delete face data',
      facesForgetTitle: 'Delete what was found about faces?',
      facesForgetText: 'Faces, people and their names are deleted, and faces are no longer looked for.',
      personTitle: 'Who is this?',
      personName: 'Name',
      personHide: 'Hide this person',
      save: 'Save',
      mergeTitle: (name) => `Merge with “${name}”?`,
      mergeText: 'There is a person with this name already. Their photos become one person.',
      merge: 'Merge',
      facesUnknown: (n) => `Faces without a person: ${n}`,
      facesFound: (n) => plural(n, `${n} face`, `${n} faces`, `${n} faces`),
      photosOf: (n) => plural(n, `${n} photo`, `${n} photos`, `${n} photos`),
      dropHint: 'Drop photos and videos here',
      dropTo: (folder) => `They will go to “${folder}”`,
      dropChoose: 'You choose the folder next',
      uploadTitle: 'Where to upload?',
      uploadButton: 'Upload',
      uploading: (done, total) => `Uploading: ${done} of ${total}`,
      uploaded: (n) => `Uploaded: ${n}`,
      uploadSkipped: (n) => `Not photos or videos, skipped: ${n}`,
      nothingToUpload: 'There are no photos or videos among these files.',
      foldersRefused: 'The browser does not let the page read dropped folders: choose their files with “Upload”.',
      dropFailed: (why) => `The dropped files could not be taken: ${why}`,
      tooLarge: 'larger than the server allows',
      noSpace: 'not enough space',
      nameTaken: 'the name is taken',
      readOnly: 'the folder is for reading only',
      linkGone: 'the link has expired, try again',
      networkError: 'the connection broke',
      nothingFound: 'Nothing found',
      nothingFoundHint: 'Try removing some of the filters.',
      emptyLibrary: 'No photos yet',
      emptyLibraryHint: 'Photos and videos appear here as soon as they are processed.',
      indexing: 'The gallery is going through your photos…',
      noFfmpeg: 'The server has no ffmpeg, or the administrator hasn’t allowed it to the gallery: there will be no thumbnails.',
      info: 'Info',
      close: 'Close',
      prev: 'Previous',
      next: 'Next',
      favorite: 'Add to favorites',
      unfavorite: 'Remove from favorites',
      download: 'Download',
      downloading: 'Downloading…',
      loading: 'Loading…',
      converting: 'The server converts the video…',
      play: 'Play (Space)',
      pause: 'Pause (Space)',
      mute: 'Mute (M)',
      unmute: 'Unmute (M)',
      volume: 'Volume (↑ ↓)',
      seek: 'Position (J, L: 10 s)',
      speed: 'Speed',
      videoFailed: 'This video can’t be played here.',
      details: 'Details',
      size: 'Size',
      folder: 'Folder',
      camera: 'Camera',
      lens: 'Lens',
      location: 'Location',
      modified: 'Changed',
      noTakenDate: 'No date in the file; shown by its last change',
      problem: 'Not processed in full',
      megapixels: 'MP',
      today: 'Today',
      yesterday: 'Yesterday',
      error: (message) => `Error: ${message}`,
    },
    ru: {
      title: 'Фото',
      search: 'Поиск по имени, папке, камере',
      items: (n) => `${n.toLocaleString('ru')} ${plural(n, 'файл', 'файла', 'файлов')}`,
      processing: (n) => `Обработка: осталось ${n.toLocaleString('ru')}`,
      failed: (n) => `Не удалось обработать: ${n}`,
      filters: 'Фильтры',
      resetAll: 'Сбросить все',
      type: 'Тип',
      all: 'Все',
      photos: 'Фото',
      videos: 'Видео',
      onlyFavorites: 'Только избранное',
      withLocation: 'С геометкой',
      period: 'Период',
      from: 'С',
      to: 'По',
      years: 'Годы',
      cameras: 'Камеры',
      lenses: 'Объективы',
      folders: 'Папки',
      formats: 'Форматы',
      orientation: 'Ориентация',
      landscape: 'Горизонтальные',
      portrait: 'Вертикальные',
      square: 'Квадратные',
      panorama: 'Панорамы',
      more: (n) => `Ещё ${n}`,
      less: 'Свернуть',
      newest: 'Сначала новые',
      oldest: 'Сначала старые',
      addFolder: 'Добавить папку',
      folderName: 'Фото',
      chooseTitle: 'Покажите, где лежат фото',
      chooseHint:
        'Галерея показывает фото и видео из папок ваших файлов. Файлы остаются на месте.',
      chooseButton: 'Выбрать папку',
      denied: 'Без доступа к папке галерее нечего показать.',
      removeHint: 'Убрать папку можно в «Настройки → Приложения с доступом», затем нажмите «Пересобрать».',
      rebuild: 'Пересобрать',
      rebuildTitle: 'Пересобрать галерею?',
      rebuildText:
        'Галерея заново пройдёт по папкам: найдёт пропущенное и уберёт то, к чему больше нет доступа. Готовые миниатюры останутся.',
      rebuildStarted: 'Пересборка: галерея проходит по папкам.',
      delete: 'Удалить (Del)',
      deleteTitle: (name) => `Удалить «${name}»?`,
      deleteText: 'Файл попадёт в корзину ваших файлов.',
      deleteButton: 'Удалить',
      needWriteTitle: 'Галерея может только читать эту папку',
      needWriteText: (folder) =>
        `Чтобы удалять файлы из папки «${folder}», разрешите галерее её изменять. В следующем окне выберите эту же папку.`,
      needWriteUploadText: (folder) =>
        `Чтобы загружать в папку «${folder}», разрешите галерее её изменять. В следующем окне выберите эту же папку.`,
      allow: 'Продолжить',
      cancel: 'Отмена',
      upload: 'Загрузить',
      select: 'Выбрать',
      selected: (n) => `Выбрано: ${n.toLocaleString('ru')}`,
      clearSelection: 'Снять выделение (Esc)',
      selectDay: 'Выбрать день',
      selectAll: 'Выбрать все (Ctrl+A)',
      deleteManyTitle: (n) => `Удалить ${n} ${plural(n, 'файл', 'файла', 'файлов')}?`,
      deleteManyText: 'Файлы попадут в корзину ваших файлов.',
      downloadingMany: (i, n) => `Скачивание: ${i} из ${n}…`,
      deleteFailed: (n) => `Не удалось удалить: ${n}`,
      people: 'Люди',
      facesOffText: 'Сервер может найти лица на ваших фото и сгруппировать их по людям. Данные о лицах остаются в ваших данных плагина.',
      facesOn: 'Найти людей',
      facesAskTitle: 'Искать людей на фото?',
      facesAskText:
        'Сервер просмотрит каждое фото нейросетью и сохранит описание каждого лица в ваших данных плагина. Их никто, кроме вас, не видит; распознавание можно выключить и всё удалить в любой момент.',
      facesDownloading: 'Сервер загружает нейросети…',
      facesNotAllowed: 'Администратор ещё не разрешил плагину нейросети (use.onnx).',
      facesNoEngine: 'На этом сервере нет движка нейросетей.',
      facesFailed: 'Не удалось запустить нейросети.',
      facesLooking: (n) => `Ищу лица: осталось ${n}`,
      facesNone: 'Людей пока нет: человек появится, когда его лицо найдётся на нескольких фото.',
      nameN: (n) => `Name ${n}`,
      attach: 'Присоединить',
      attachTitle: 'Присоединить к',
      attachWhole: (n) => `Все лица этого человека (${n} фото)`,
      notThem: 'Это не он',
      avatarTitle: 'Лицо для аватара',
      newPersonHint: 'Имя сделает из этого лица нового человека; или присоедините его к кому-то.',
      showFaces: 'Лица',
      showHidden: (n) => `Скрытые: ${n}`,
      hideHidden: 'Снова скрыть',
      facesOff: 'Выключить',
      facesForget: 'Удалить данные о лицах',
      facesForgetTitle: 'Удалить всё найденное о лицах?',
      facesForgetText: 'Лица, люди и их имена будут удалены, а лица больше не будут искаться.',
      personTitle: 'Кто это?',
      personName: 'Имя',
      personHide: 'Скрыть этого человека',
      save: 'Сохранить',
      mergeTitle: (name) => `Объединить с «${name}»?`,
      mergeText: 'Человек с таким именем уже есть. Его фото станут фото одного человека.',
      merge: 'Объединить',
      facesUnknown: (n) => `Лиц без человека: ${n}`,
      facesFound: (n) => plural(n, `${n} лицо`, `${n} лица`, `${n} лиц`),
      photosOf: (n) => plural(n, `${n} фото`, `${n} фото`, `${n} фото`),
      dropHint: 'Отпустите, чтобы загрузить фото и видео',
      dropTo: (folder) => `Они попадут в папку «${folder}»`,
      dropChoose: 'Папку выберете дальше',
      uploadTitle: 'Куда загрузить?',
      uploadButton: 'Загрузить',
      uploading: (done, total) => `Загрузка: ${done} из ${total}`,
      uploaded: (n) => `Загружено: ${n}`,
      uploadSkipped: (n) => `Не фото и не видео, пропущено: ${n}`,
      nothingToUpload: 'Среди этих файлов нет фото и видео.',
      foldersRefused: 'Браузер не даёт странице прочитать перетащенные папки: выберите их файлы кнопкой «Загрузить».',
      dropFailed: (why) => `Не удалось взять перетащенные файлы: ${why}`,
      tooLarge: 'больше, чем разрешает сервер',
      noSpace: 'не хватает места',
      nameTaken: 'имя занято',
      readOnly: 'папка только для чтения',
      linkGone: 'ссылка устарела, попробуйте ещё раз',
      networkError: 'оборвалась связь',
      nothingFound: 'Ничего не найдено',
      nothingFoundHint: 'Попробуйте убрать часть фильтров.',
      emptyLibrary: 'Фото пока нет',
      emptyLibraryHint: 'Фото и видео появятся здесь, как только будут обработаны.',
      indexing: 'Галерея просматривает ваши фото…',
      noFfmpeg: 'На сервере нет ffmpeg или администратор не разрешил его галерее: миниатюр не будет.',
      info: 'Сведения',
      close: 'Закрыть',
      prev: 'Предыдущее',
      next: 'Следующее',
      favorite: 'В избранное',
      unfavorite: 'Убрать из избранного',
      download: 'Скачать',
      downloading: 'Скачивание…',
      loading: 'Загрузка…',
      converting: 'Сервер перекодирует видео…',
      play: 'Смотреть (пробел)',
      pause: 'Пауза (пробел)',
      mute: 'Выключить звук (M)',
      unmute: 'Включить звук (M)',
      volume: 'Громкость (↑ ↓)',
      seek: 'Позиция (J, L — на 10 с)',
      speed: 'Скорость',
      videoFailed: 'Это видео здесь не воспроизвести.',
      details: 'Сведения',
      size: 'Размер',
      folder: 'Папка',
      camera: 'Камера',
      lens: 'Объектив',
      location: 'Место',
      modified: 'Изменён',
      noTakenDate: 'Даты съёмки в файле нет; показан по последнему изменению',
      problem: 'Обработан не полностью',
      megapixels: 'Мп',
      today: 'Сегодня',
      yesterday: 'Вчера',
      error: (message) => `Ошибка: ${message}`,
    },
  };

  /** Russian plural: 1 файл, 2 файла, 5 файлов. */
  function plural(n, one, few, many) {
    const rule = new Intl.PluralRules('ru').select(n);
    return rule === 'one' ? one : rule === 'few' ? few : many;
  }

  const ICONS = {
    search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
    filter: '<path d="M4 6h16M7 12h10M10 18h4"/>',
    sort: '<path d="M7 4v16M3 16l4 4 4-4M17 20V4M13 8l4-4 4 4"/>',
    plus: '<path d="M12 5v14M5 12h14"/>',
    more: '<circle cx="5" cy="12" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="19" cy="12" r="1.2"/>',
    close: '<path d="M6 6l12 12M18 6 6 18"/>',
    left: '<path d="m15 5-7 7 7 7"/>',
    right: '<path d="m9 5 7 7-7 7"/>',
    info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7.5v.01"/>',
    heart: '<path d="M12 20s-7-4.4-7-10a4 4 0 0 1 7-2.6A4 4 0 0 1 19 10c0 5.6-7 10-7 10z"/>',
    download: '<path d="M12 4v11M7 10l5 5 5-5M5 20h14"/>',
    upload: '<path d="M12 16V5M7 10l5-5 5 5M5 20h14"/>',
    check: '<path d="m5 12 5 5 9-10"/>',
    person: '<circle cx="12" cy="8" r="4"/><path d="M4 20c1.5-4 4.5-6 8-6s6.5 2 8 6"/>',
    selectAll: '<rect x="3" y="3" width="18" height="18" rx="3"/><path d="m8 12 3 3 5-6"/>',
    trash: '<path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/>',
    play: '<path d="M8 5v14l11-7z"/>',
    pause: '<rect x="6" y="5" width="4" height="14" rx="1"/><rect x="14" y="5" width="4" height="14" rx="1"/>',
    volume: '<path d="M4 9v6h4l5 4V5L8 9z"/><path d="M16.5 8.5a5 5 0 0 1 0 7M19 6a8.5 8.5 0 0 1 0 12"/>',
    muted: '<path d="M4 9v6h4l5 4V5L8 9z"/><path d="m17 9 5 6M22 9l-5 6"/>',
    image: '<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="2"/><path d="m21 16-5-5-9 9"/>',
    video: '<rect x="3" y="5" width="14" height="14" rx="2"/><path d="m17 10 4-2v8l-4-2"/>',
    calendar: '<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M3 10h18M8 3v4M16 3v4"/>',
    folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
    camera: '<path d="M4 8h3l2-3h6l2 3h3v11H4z"/><circle cx="12" cy="13" r="3.5"/>',
    aperture: '<circle cx="12" cy="12" r="9"/><path d="M12 3l3 7M21 12l-7 3M12 21l-3-7M3 12l7-3"/>',
    pin: '<path d="M12 21s-6-5.7-6-11a6 6 0 0 1 12 0c0 5.3-6 11-6 11z"/><circle cx="12" cy="10" r="2.2"/>',
    file: '<path d="M6 3h8l4 4v14H6z"/><path d="M14 3v4h4"/>',
    alert: '<path d="M12 4 2.5 20h19z"/><path d="M12 10v4M12 17v.01"/>',
    photos: '<rect x="3" y="3" width="8" height="8" rx="2"/><rect x="13" y="3" width="8" height="8" rx="2"/><rect x="3" y="13" width="8" height="8" rx="2"/><rect x="13" y="13" width="8" height="8" rx="2"/>',
  };

  const context = await rustcloud.context;
  const t = TEXTS[context.locale] ?? TEXTS.en;
  const intlLocale = context.locale === 'ru' ? 'ru-RU' : 'en-US';
  document.documentElement.dataset.theme = context.theme;
  document.documentElement.lang = context.locale;
  document.title = t.title;
  const app = document.getElementById('app');

  // ── Small helpers ──────────────────────────────────────────────────

  /** An element: h('div', { class: 'x', onclick }, child, 'text'). */
  function h(tag, attrs = {}, ...children) {
    const el = document.createElement(tag);
    for (const [key, value] of Object.entries(attrs)) {
      if (value === undefined || value === null || value === false) continue;
      if (key.startsWith('on')) el.addEventListener(key.slice(2), value);
      else if (key === 'class') el.className = value;
      else if (key === 'style') el.style.cssText = value;
      else el.setAttribute(key, value === true ? '' : value);
    }
    for (const child of children.flat()) {
      if (child === null || child === undefined || child === false) continue;
      el.append(child instanceof Node ? child : document.createTextNode(String(child)));
    }
    return el;
  }

  function icon(name) {
    const span = document.createElement('span');
    span.innerHTML = `<svg class="icon" viewBox="0 0 24 24" aria-hidden="true">${ICONS[name]}</svg>`;
    return span.firstChild;
  }

  function iconButton(name, label, onclick, extra = {}) {
    return h('button', { class: 'icon-btn', title: label, 'aria-label': label, onclick, ...extra }, icon(name));
  }

  const pad = (n) => String(n).padStart(2, '0');

  function bytes(n) {
    const units = context.locale === 'ru' ? ['Б', 'КБ', 'МБ', 'ГБ', 'ТБ'] : ['B', 'KB', 'MB', 'GB', 'TB'];
    let i = 0;
    while (n >= 1024 && i < units.length - 1) {
      n /= 1024;
      i++;
    }
    return `${n.toLocaleString(intlLocale, { maximumFractionDigits: i ? 1 : 0 })} ${units[i]}`;
  }

  function durationText(seconds) {
    return duration(seconds * 1000);
  }

  function duration(ms) {
    const s = Math.round(ms / 1000);
    const hours = Math.floor(s / 3600);
    const rest = `${pad(Math.floor((s % 3600) / 60))}:${pad(s % 60)}`;
    return hours ? `${hours}:${rest}` : rest.replace(/^0/, '');
  }

  /** A local date-time from `YYYY-MM-DDTHH:MM:SS`. */
  function localDate(when) {
    const [y, m, d, hh, mm, ss] = when.split(/[-T:]/).map(Number);
    return new Date(y, m - 1, d, hh || 0, mm || 0, ss || 0);
  }

  const monthFormat = new Intl.DateTimeFormat(intlLocale, { month: 'long', year: 'numeric' });
  const dayFormat = new Intl.DateTimeFormat(intlLocale, { weekday: 'short', day: 'numeric', month: 'long' });
  const dayYearFormat = new Intl.DateTimeFormat(intlLocale, {
    weekday: 'short',
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  });
  const fullFormat = new Intl.DateTimeFormat(intlLocale, { dateStyle: 'full', timeStyle: 'short' });

  function capitalize(text) {
    return text.charAt(0).toUpperCase() + text.slice(1);
  }

  function monthTitle(month) {
    return capitalize(monthFormat.format(localDate(`${month}-01`)));
  }

  function dayTitle(day) {
    const today = new Date();
    const key = (date) => `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
    if (day === key(today)) return t.today;
    if (day === key(new Date(today.getFullYear(), today.getMonth(), today.getDate() - 1))) return t.yesterday;
    const date = localDate(day);
    const format = date.getFullYear() === today.getFullYear() ? dayFormat : dayYearFormat;
    return capitalize(format.format(date));
  }

  // ── Requests ───────────────────────────────────────────────────────

  async function api(method, path, body) {
    const init = { method };
    if (body !== undefined) {
      init.body = JSON.stringify(body);
      init.headers = { 'content-type': 'application/json' };
    }
    const response = await rustcloud.fetch('/api' + path, init);
    const data = await response.json().catch(() => ({}));
    if (!response.ok) {
      const error = new Error(data.error || `HTTP ${response.status}`);
      error.status = response.status;
      error.code = data.code;
      throw error;
    }
    return data;
  }

  /**
   * Links to thumbnails, previews and originals, asked for in batches. The
   * key includes the file's etag: a changed file gets a new link.
   */
  const links = new Map();
  let linkQueue = new Map();
  let linkTimer = null;

  function link(kind, entry) {
    const key = `${kind}:${entry.id}:${entry.etag}`;
    if (links.has(key)) return links.get(key);
    const promise = new Promise((resolve) => {
      linkQueue.set(key, { id: entry.id, kind, resolve });
    });
    links.set(key, promise);
    linkTimer ??= setTimeout(flushLinks, 20);
    return promise;
  }

  async function flushLinks() {
    linkTimer = null;
    const queued = [...linkQueue.entries()];
    linkQueue = new Map();
    for (let i = 0; i < queued.length; i += 500) {
      const part = queued.slice(i, i + 500);
      try {
        const { urls } = await api('POST', '/links', { items: part.map(([, item]) => ({ id: item.id, kind: item.kind })) });
        part.forEach(([, item], n) => item.resolve(urls[n] ?? null));
      } catch {
        // Ask again next time rather than keeping the failure.
        for (const [key, item] of part) {
          links.delete(key);
          item.resolve(null);
        }
      }
    }
  }

  // ── Library ────────────────────────────────────────────────────────

  let folders = [];
  let favorites = new Set();
  /** The folder uploads went to last, kept by the plugin. */
  let uploadFolderId = null;
  let progress = {};
  let ffmpeg = true;
  /** Faces: `{enabled, state}` from the plugin. */
  let faces = { enabled: false };
  /** People as the last grouping left them, by id; names are the user's. */
  let people = new Map();
  let names = { names: {}, hidden: [], merged: {} };
  let peopleUrl = null;
  /** Entries of the library by id. */
  let byId = new Map();
  /** The faces of photos, by id and etag, as the viewer asked for them. */
  const facesCache = new Map();
  function facesOf(entry) {
    const key = `${entry.id}:${entry.etag}`;
    if (!facesCache.has(key)) {
      facesCache.set(
        key,
        api('GET', `/faces/${entry.id}`).then(
          (answer) => answer.faces,
          () => (facesCache.delete(key), []),
        ),
      );
    }
    return facesCache.get(key);
  }
  /** Whether the viewer draws circles on faces. */
  let facesShown = true;
  /** The photos of each person filtered by, as people and names are now. */
  let personFilesCache = new Map();
  let showHiddenPeople = false;
  let loaded = false;
  let problem = '';
  /** Shards by link: the browser caches them, the page parses each once. */
  let shardCache = new Map();
  /** Everything in the index that is in a granted folder. */
  let library = [];
  /** Files deleted from the page, hidden until the index drops them. */
  const deletedIds = new Set();

  /** Dates, labels and search text an entry is filtered and grouped by. */
  function prepare(entry) {
    let when = '';
    if (entry.taken && /^\d{4}-\d{2}-\d{2}/.test(entry.taken)) {
      when = entry.taken.slice(0, 19).replace(' ', 'T');
    } else {
      const date = new Date(entry.modified);
      if (!Number.isNaN(date.getTime())) {
        when = `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
      } else {
        when = '1970-01-01T00:00:00';
      }
    }
    if (when.length === 10) when += 'T00:00:00';
    entry._when = when;
    entry._day = when.slice(0, 10);
    entry._month = when.slice(0, 7);
    entry._year = when.slice(0, 4);
    entry._format = formatOf(entry);
    entry._orientation = orientationOf(entry);
    entry._search = [entry.name, entry.folder, entry.camera, entry.lens].filter(Boolean).join(' ').toLowerCase();
    return entry;
  }

  function formatOf(entry) {
    const named = {
      'image/jpeg': 'JPEG',
      'image/png': 'PNG',
      'image/heic': 'HEIC',
      'image/heif': 'HEIC',
      'image/avif': 'AVIF',
      'image/webp': 'WebP',
      'image/gif': 'GIF',
      'image/tiff': 'TIFF',
      'image/bmp': 'BMP',
    }[entry.mime];
    if (named) return named;
    const extension = entry.name.includes('.') ? entry.name.split('.').pop() : '';
    return (entry.container && !entry.container.includes(',') ? entry.container : extension).toUpperCase() || '?';
  }

  function orientationOf(entry) {
    if (!entry.width || !entry.height) return null;
    const ratio = entry.width / entry.height;
    if (ratio >= 2 || ratio <= 0.5) return 'panorama';
    if (ratio > 1.05) return 'landscape';
    if (ratio < 0.95) return 'portrait';
    return 'square';
  }

  /** A shard was rewritten after its link was made: that link is closed. */
  class ShardChanged extends Error {}

  async function loadState() {
    // A job may rewrite a shard between the links and the fetch: then again.
    for (let attempt = 1; ; attempt++) {
      try {
        return await loadStateOnce();
      } catch (e) {
        if (!(e instanceof ShardChanged) || attempt === 3) throw e;
      }
    }
  }

  async function loadStateOnce() {
    const state = await api('GET', '/state');
    folders = state.folders;
    favorites = new Set(state.favorites);
    uploadFolderId = state.uploadFolder ?? null;
    progress = state.progress || {};
    ffmpeg = state.ffmpeg;
    faces = state.faces ?? { enabled: false };
    names = state.names ?? names;
    await loadPeople(state.people);
    personFilesCache = new Map();
    const next = new Map();
    await Promise.all(
      state.shards.map(async (url) => {
        if (shardCache.has(url)) {
          next.set(url, shardCache.get(url));
          return;
        }
        const response = await fetch(url);
        if (response.status === 404) throw new ShardChanged('HTTP 404');
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        next.set(url, (await response.json()).map(prepare));
      }),
    );
    shardCache = next;
    const roots = new Set(folders.map((folder) => folder.id));
    const all = [...next.values()].flat();
    // Deleted here, but the index hasn't caught up yet.
    const present = new Set(all.map((entry) => entry.id));
    for (const id of deletedIds) if (!present.has(id)) deletedIds.delete(id);
    library = all.filter((entry) => roots.has(entry.root) && !deletedIds.has(entry.id));
    byId = new Map(library.map((entry) => [entry.id, entry]));
    loaded = true;
  }

  /** The people file changes after each grouping: a new link each time. */
  async function loadPeople(url) {
    if (!url) {
      people = new Map();
      peopleUrl = null;
      return;
    }
    if (url === peopleUrl) return;
    try {
      const response = await fetch(url);
      // Rewritten meanwhile: the next poll gets the new one.
      if (!response.ok) return;
      const data = await response.json();
      people = new Map(data.people.map((person) => [person.id, { ...person, files: new Set(person.files) }]));
      peopleUrl = url;
    } catch {
      // Kept as it was.
    }
  }

  /** Where a merged person went (the grouping catches up later). */
  function personOf(id) {
    for (let i = 0; i < 16 && names.merged[id]; i++) id = names.merged[id];
    return id;
  }

  /** The user's name for a person, or `Name <n>` by their number. */
  function personName(id) {
    id = personOf(id);
    if (names.names[id]) return names.names[id];
    const n = Number(id.slice(1));
    return id.startsWith('m') ? t.nameN(`M${n + 1}`) : t.nameN(n + 1);
  }

  /** People to show: merges applied, the most photos first. */
  function peopleList() {
    const merged = new Map();
    for (const person of people.values()) {
      const id = personOf(person.id);
      const into = merged.get(id);
      if (into) {
        for (const file of person.files) into.files.add(file);
        into.count += person.count;
      } else {
        merged.set(id, { ...person, id, files: new Set(person.files) });
      }
    }
    // People made by hand show at once, before the grouping catches up.
    for (const id of Object.keys(names.names)) {
      if (id.startsWith('m') && !merged.has(id) && !names.merged[id]) merged.set(id, { id, count: 0, files: new Set(), cover: null });
    }
    return [...merged.values()].sort((a, b) => b.files.size - a.files.size || a.id.localeCompare(b.id));
  }

  const waiting = () => Object.values(progress).reduce((sum, kind) => sum + kind.waiting, 0);
  const failedCount = () => progress.media?.failed ?? 0;

  /** Reloads the state while the jobs run, so new photos appear as they're done. */
  let pollTimer = null;
  function schedulePoll() {
    clearTimeout(pollTimer);
    if (!waiting()) return;
    pollTimer = setTimeout(async () => {
      try {
        await loadState();
        problem = '';
      } catch (e) {
        problem = t.error(e.message);
      }
      refresh();
      schedulePoll();
    }, 4000);
  }

  // ── Filters ────────────────────────────────────────────────────────

  const filters = {
    q: '',
    type: 'all',
    favorites: false,
    gps: false,
    from: '',
    to: '',
    years: new Set(),
    cameras: new Set(),
    lenses: new Set(),
    folders: new Set(),
    formats: new Set(),
    orientation: new Set(),
    /** Person ids: photos with all of them. */
    people: new Set(),
  };
  let sortNewest = true;

  /** Filters with a list of values, and what each entry has for them. */
  const FACETS = {
    years: (e) => e._year,
    cameras: (e) => e.camera,
    lenses: (e) => e.lens,
    formats: (e) => e._format,
    orientation: (e) => e._orientation,
  };

  /** A folder filter also takes the folders inside it. */
  function inFolders(entry, chosen) {
    for (const folder of chosen) {
      if (entry.folder === folder || entry.folder.startsWith(folder + '/')) return true;
    }
    return false;
  }

  /** Whether the entry passes every filter but `skip`. */
  function passes(entry, skip) {
    if (filters.type === 'photo' && entry.video) return false;
    if (filters.type === 'video' && !entry.video) return false;
    if (filters.favorites && !favorites.has(entry.id)) return false;
    if (filters.gps && !entry.gps) return false;
    if (filters.from && entry._day < filters.from) return false;
    if (filters.to && entry._day > filters.to) return false;
    if (filters.q && !filters.q.split(/\s+/).every((word) => entry._search.includes(word))) return false;
    for (const id of filters.people) if (!personFiles(id).has(entry.id)) return false;
    if (skip !== 'folders' && filters.folders.size && !inFolders(entry, filters.folders)) return false;
    for (const [name, value] of Object.entries(FACETS)) {
      if (name !== skip && filters[name].size && !filters[name].has(value(entry))) return false;
    }
    return true;
  }

  /** The photos of a person, those of people merged into them too. */
  function personFiles(id) {
    let files = personFilesCache.get(id);
    if (!files) {
      files = new Set();
      for (const person of people.values()) {
        if (personOf(person.id) === id) for (const file of person.files) files.add(file);
      }
      personFilesCache.set(id, files);
    }
    return files;
  }

  function activeFilters() {
    const active = [];
    if (filters.type !== 'all') active.push(['type', filters.type === 'photo' ? t.photos : t.videos]);
    if (filters.favorites) active.push(['favorites', t.onlyFavorites]);
    if (filters.gps) active.push(['gps', t.withLocation]);
    if (filters.from) active.push(['from', `${t.from} ${filters.from}`]);
    if (filters.to) active.push(['to', `${t.to} ${filters.to}`]);
    for (const id of filters.people) active.push(['people', personName(id), id]);
    for (const name of ['years', 'cameras', 'lenses', 'folders', 'formats', 'orientation']) {
      for (const value of filters[name]) active.push([name, name === 'orientation' ? t[value] : value, value]);
    }
    return active;
  }

  function clearFilter(name, value) {
    if (name === 'type') filters.type = 'all';
    else if (name === 'favorites' || name === 'gps') filters[name] = false;
    else if (name === 'from' || name === 'to') filters[name] = '';
    else filters[name].delete(value);
  }

  function clearAll() {
    for (const [name, value] of activeFilters()) clearFilter(name, value);
  }

  /** How many entries each value of a facet has, the other filters applied. */
  function facetCounts(name) {
    const counts = new Map();
    for (const entry of library) {
      if (!passes(entry, name)) continue;
      if (name === 'folders') {
        // Each folder counts what is in it and below it.
        const parts = entry.folder.split('/');
        for (let i = 1; i <= parts.length; i++) {
          const path = parts.slice(0, i).join('/');
          counts.set(path, (counts.get(path) ?? 0) + 1);
        }
      } else {
        const value = FACETS[name](entry);
        if (value) counts.set(value, (counts.get(value) ?? 0) + 1);
      }
    }
    for (const value of filters[name]) if (!counts.has(value)) counts.set(value, 0);
    return counts;
  }

  /** What the timeline shows, in order. */
  let shown = [];
  /** The ids of the shown entries by day, for selecting a day. */
  let dayIds = new Map();
  /** Selected ids; while there are any, the top bar acts on them. */
  const selected = new Set();
  /** Where a Shift-click range starts: an index into `shown`. */
  let selectAnchor = null;
  /** What the selection bar says while it works (downloads). */
  let selectionStatus = '';

  function applyFilters() {
    shown = library.filter((entry) => passes(entry));
    shown.sort((a, b) => (a._when === b._when ? a.id.localeCompare(b.id) : a._when < b._when ? -1 : 1));
    if (sortNewest) shown.reverse();
  }

  // ── Page ───────────────────────────────────────────────────────────

  let filtersOpen = innerWidth > 900;
  const expanded = new Set();
  let notice = '';

  const timeline = h('div', { class: 'timeline' });
  const canvas = h('div', { class: 'canvas' });
  timeline.append(canvas);
  const scrubber = h('div', { class: 'scrubber' });
  const scrubLabel = h('div', { class: 'scrub-label' });
  const timelineWrap = h('div', { class: 'timeline-wrap' }, timeline, scrubber, scrubLabel);
  const searchInput = h('input', { type: 'search', placeholder: t.search, 'aria-label': t.search });
  let searchTimer = null;
  searchInput.addEventListener('input', () => {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      filters.q = searchInput.value.trim().toLowerCase();
      refresh({ keepScroll: false });
    }, 150);
  });

  // The page's parts; refresh() swaps the ones that changed and leaves the
  // timeline in place (its scroll position, its pictures) unless what it
  // shows changed.
  let topEl = h('div');
  let chipsEl = h('div');
  let noticesEl = h('div');
  let mainEl = h('div');
  let filtersEl = h('div');
  const bodyEl = h('div', { class: 'body' }, mainEl, filtersEl);
  app.append(topEl, chipsEl, noticesEl, bodyEl);
  let shownKey = '';

  /** Redraws the page (not the viewer). */
  function refresh({ keepScroll = true } = {}) {
    const anchor = keepScroll ? anchorItem() : null;
    applyFilters();
    dayIds = new Map();
    for (const entry of shown) {
      if (!dayIds.has(entry._day)) dayIds.set(entry._day, []);
      dayIds.get(entry._day).push(entry.id);
    }
    // What is no longer shown is no longer selected.
    if (selected.size) {
      const visible = new Set(shown.map((entry) => entry.id));
      for (const id of selected) if (!visible.has(id)) selected.delete(id);
    }
    canvas.classList.toggle('selecting', selected.size > 0);
    const searching = document.activeElement === searchInput;
    const caret = [searchInput.selectionStart, searchInput.selectionEnd];
    topEl = swap(topEl, topbar());
    chipsEl = swap(chipsEl, chips());
    noticesEl = swap(noticesEl, h('div', { style: 'display:contents' }, notices()));
    const filtersScroll = filtersEl.scrollTop;
    filtersEl = swap(filtersEl, loaded && folders.length && filtersOpen ? filtersPanel() : h('div', { hidden: true }));
    filtersEl.scrollTop = filtersScroll;
    if (searching) {
      searchInput.focus();
      searchInput.setSelectionRange(...caret);
    }
    const main = mainView();
    if (main !== mainEl) mainEl = swap(mainEl, main);
    // What the timeline shows: a new key means a new layout.
    const key = shown.map((e) => `${e.id}:${e.etag}:${e.thumb ? 1 : 0}`).join(',');
    if (main === timelineWrap && (key !== shownKey || !keepScroll)) {
      shownKey = key;
      relayout(anchor);
    }
  }

  function swap(old, fresh) {
    old.replaceWith(fresh);
    return fresh;
  }

  function topbar() {
    if (selected.size) return selectionBar();
    const left = waiting();
    const count = activeFilters().length;
    return h(
      'header',
      { class: 'topbar' },
      h('h1', {}, t.title),
      loaded && folders.length ? h('span', { class: 'count' }, t.items(shown.length)) : null,
      h('label', { class: 'search' }, icon('search'), searchInput),
      h('div', { class: 'spacer' }),
      left ? h('span', { class: 'progress-chip' }, h('span', { class: 'spinner' }), t.processing(left)) : null,
      iconButton('sort', sortNewest ? t.newest : t.oldest, () => {
        sortNewest = !sortNewest;
        refresh({ keepScroll: false });
      }),
      h(
        'button',
        {
          class: 'icon-btn',
          title: t.filters,
          'aria-label': t.filters,
          'aria-pressed': String(filtersOpen),
          onclick: () => {
            filtersOpen = !filtersOpen;
            refresh();
          },
        },
        icon('filter'),
        count ? h('span', { class: 'badge' }, count) : null,
      ),
      folders.length ? iconButton('upload', t.upload, pickFiles) : null,
      folders.length ? iconButton('plus', t.addFolder, addFolder) : null,
      folders.length ? iconButton('more', t.rebuild, confirmRebuild) : null,
    );
  }

  function chips() {
    const active = activeFilters();
    return h(
      'div',
      { class: 'chips' },
      active.map(([name, label, value]) =>
        h(
          'button',
          {
            class: 'chip',
            onclick: () => {
              clearFilter(name, value);
              refresh({ keepScroll: false });
            },
          },
          label,
          icon('close'),
        ),
      ),
      active.length > 1
        ? h(
            'button',
            {
              class: 'chip clear',
              onclick: () => {
                clearAll();
                refresh({ keepScroll: false });
              },
            },
            t.resetAll,
          )
        : null,
    );
  }

  function notices() {
    const list = [];
    if (problem) list.push(h('div', { class: 'notice error' }, problem));
    if (notice) list.push(h('div', { class: 'notice' }, notice));
    if (loaded && folders.length && !ffmpeg) list.push(h('div', { class: 'notice' }, t.noFfmpeg));
    if (failedCount()) list.push(h('div', { class: 'notice' }, t.failed(failedCount())));
    return list;
  }

  /** The timeline, or a card saying why there is none. */
  function mainView() {
    if (!loaded) return h('div', { class: 'empty' }, h('div', { class: 'spinner' }));
    if (!folders.length) {
      const choose = h('button', { class: 'btn primary', onclick: addFolder }, t.chooseButton);
      return emptyCard('photos', t.chooseTitle, t.chooseHint, choose);
    }
    if (!library.length) {
      return waiting()
        ? emptyCard('image', t.indexing, t.emptyLibraryHint, h('div', { class: 'spinner' }))
        : emptyCard('image', t.emptyLibrary, t.emptyLibraryHint);
    }
    if (!shown.length) {
      const reset = () => {
        clearAll();
        refresh({ keepScroll: false });
      };
      return emptyCard('search', t.nothingFound, t.nothingFoundHint, h('button', { class: 'btn', onclick: reset }, t.resetAll));
    }
    return timelineWrap;
  }

  function emptyCard(art, title, text, ...actions) {
    return h(
      'div',
      { class: 'empty' },
      h('div', { class: 'card' }, h('div', { class: 'art' }, icon(art)), h('h2', {}, title), h('p', {}, text), ...actions),
    );
  }

  // ── Filters panel ──────────────────────────────────────────────────

  function filtersPanel() {
    const change = () => refresh({ keepScroll: false });
    const typeButton = (value, label) =>
      h(
        'button',
        {
          class: filters.type === value ? 'on' : '',
          onclick: () => {
            filters.type = value;
            change();
          },
        },
        label,
      );
    const toggle = (name, label) =>
      h(
        'label',
        { class: 'toggle-row' },
        h('span', {}, label),
        h('input', {
          type: 'checkbox',
          class: 'switch',
          checked: filters[name],
          onchange: (event) => {
            filters[name] = event.target.checked;
            change();
          },
        }),
      );
    const date = (name, label) =>
      h(
        'label',
        {},
        label,
        h('input', {
          type: 'date',
          value: filters[name],
          onchange: (event) => {
            filters[name] = event.target.value;
            change();
          },
        }),
      );
    const pick = (name, value) => {
      if (filters[name].has(value)) filters[name].delete(value);
      else filters[name].add(value);
      change();
    };

    const years = [...facetCounts('years')].sort((a, b) => b[0].localeCompare(a[0]));
    const orientations = facetCounts('orientation');

    return h(
      'aside',
      { class: 'filters', 'aria-label': t.filters },
      h('header', {}, h('h2', {}, t.filters), iconButton('close', t.close, () => ((filtersOpen = false), refresh()))),
      h(
        'section',
        {},
        h('h3', {}, t.type),
        h('div', { class: 'segmented' }, typeButton('all', t.all), typeButton('photo', t.photos), typeButton('video', t.videos)),
      ),
      h('section', {}, toggle('favorites', t.onlyFavorites), toggle('gps', t.withLocation)),
      peopleSection(),
      h('section', {}, h('h3', {}, t.period), h('div', { class: 'dates' }, date('from', t.from), date('to', t.to))),
      years.length
        ? h(
            'section',
            {},
            h('h3', {}, t.years),
            h(
              'div',
              { class: 'pills' },
              years.map(([year, n]) =>
                h(
                  'button',
                  { class: 'pill' + (filters.years.has(year) ? ' on' : ''), onclick: () => pick('years', year) },
                  year,
                  h('span', { class: 'n' }, n),
                ),
              ),
            ),
          )
        : null,
      checkSection('cameras', t.cameras, pick),
      checkSection('lenses', t.lenses, pick),
      foldersSection(pick),
      checkSection('formats', t.formats, pick),
      orientations.size
        ? h(
            'section',
            {},
            h('h3', {}, t.orientation),
            h(
              'div',
              { class: 'pills' },
              ['landscape', 'portrait', 'square', 'panorama']
                .filter((value) => orientations.has(value))
                .map((value) =>
                  h(
                    'button',
                    { class: 'pill' + (filters.orientation.has(value) ? ' on' : ''), onclick: () => pick('orientation', value) },
                    t[value],
                    h('span', { class: 'n' }, orientations.get(value)),
                  ),
                ),
            ),
          )
        : null,
    );
  }

  /** People: a face per person to filter by, or how to turn faces on. */
  function peopleSection() {
    const title = h('h3', {}, t.people);
    if (!faces.enabled) {
      return h(
        'section',
        {},
        title,
        h('p', { class: 'hint', style: 'margin-top:0' }, t.facesOffText),
        h('button', { class: 'btn', style: 'margin-top:10px', onclick: turnFacesOn }, icon('person'), t.facesOn),
      );
    }
    const states = { downloading: t.facesDownloading, 'not-allowed': t.facesNotAllowed, 'no-engine': t.facesNoEngine };
    const left = (progress.faces?.waiting ?? 0) + (progress.people?.waiting ?? 0);
    const all = peopleList();
    const hidden = new Set(names.hidden);
    const list = all.filter((person) => showHiddenPeople || !hidden.has(person.id));
    const hiddenCount = all.length - all.filter((person) => !hidden.has(person.id)).length;
    const limit = 12;
    const open = expanded.has('people');
    return h(
      'section',
      {},
      title,
      faces.state !== 'ready' ? h('p', { class: 'hint', style: 'margin-top:0' }, states[faces.state] ?? t.facesFailed) : null,
      left ? h('p', { class: 'hint faces-progress', style: 'margin-top:0' }, h('span', { class: 'spinner' }), t.facesLooking(left)) : null,
      faces.state === 'ready' && !all.length && !left ? h('p', { class: 'hint', style: 'margin-top:0' }, t.facesNone) : null,
      list.length
        ? h(
            'div',
            { class: 'people' },
            (open ? list : list.slice(0, limit)).map((person) =>
              h(
                'div',
                { class: 'person' + (filters.people.has(person.id) ? ' on' : '') + (hidden.has(person.id) ? ' hidden' : '') },
                h(
                  'button',
                  {
                    class: 'face',
                    title: `${personName(person.id)} · ${t.photosOf(person.files.size)}`,
                    'aria-pressed': String(filters.people.has(person.id)),
                    onclick: () => {
                      if (filters.people.has(person.id)) filters.people.delete(person.id);
                      else filters.people.add(person.id);
                      refresh({ keepScroll: false });
                    },
                  },
                  avatar(person),
                ),
                h('button', { class: 'name', onclick: () => editPerson({ person }) }, personName(person.id)),
              ),
            ),
          )
        : null,
      list.length > limit
        ? h(
            'button',
            {
              class: 'more',
              onclick: () => {
                if (open) expanded.delete('people');
                else expanded.add('people');
                refresh();
              },
            },
            open ? t.less : t.more(list.length - limit),
          )
        : null,
      h(
        'div',
        { class: 'people-actions' },
        hiddenCount
          ? h(
              'button',
              { class: 'more', onclick: () => ((showHiddenPeople = !showHiddenPeople), refresh()) },
              showHiddenPeople ? t.hideHidden : t.showHidden(hiddenCount),
            )
          : null,
        h('button', { class: 'more', onclick: () => setFaces(false) }, t.facesOff),
        h('button', { class: 'more danger', onclick: forgetFaces }, t.facesForget),
      ),
    );
  }

  /** A person's face: the one the user chose, or the sharpest. */
  const AVATAR = 56;
  function avatar(person, size = AVATAR) {
    const chosen = names.covers?.[person.id];
    const face = chosen && byId.has(chosen.file) ? chosen : person.cover;
    if (!face) return h('span', { class: 'avatar', style: `width:${size}px;height:${size}px` }, icon('person'));
    return faceImage(face, size);
  }

  /** A face (`{file, b}`) cut out of the photo's thumbnail. */
  function faceImage(face, size = AVATAR) {
    const el = h('span', { class: 'avatar', style: `width:${size}px;height:${size}px` }, icon('person'));
    const entry = byId.get(face.file);
    if (!entry?.thumb) return el;
    const [x, y, w, h_] = face.b;
    const width = entry.width || 1000;
    const height = entry.height || 1000;
    // A square around the face, a little larger than it.
    const side = Math.max(w * width, h_ * height) * 1.5;
    const cx = (x + w / 2) * width;
    const cy = (y + h_ / 2) * height;
    const k = size / side;
    link('thumb', entry).then((url) => {
      if (!url) return;
      el.replaceChildren();
      Object.assign(el.style, {
        backgroundImage: `url("${url}")`,
        backgroundSize: `${width * k}px ${height * k}px`,
        backgroundPosition: `${-(cx - side / 2) * k}px ${-(cy - side / 2) * k}px`,
      });
    });
    return el;
  }

  async function turnFacesOn() {
    if (!(await ask({ title: t.facesAskTitle, text: t.facesAskText, action: t.facesOn }))) return;
    await setFaces(true);
  }

  async function setFaces(on) {
    try {
      faces = await api('PUT', '/faces', { enabled: on });
    } catch (e) {
      problem = t.error(e.message);
    }
    await reload();
  }

  async function forgetFaces() {
    if (!(await ask({ title: t.facesForgetTitle, text: t.facesForgetText, action: t.facesForget, danger: true }))) return;
    try {
      faces = await api('DELETE', '/faces');
      filters.people.clear();
    } catch (e) {
      problem = t.error(e.message);
    }
    await reload();
  }

  /**
   * The editor of a person, or of one face (`face`: `{file, i, b, p}`, from
   * the viewer): name, hide, the face to show; attach the face or the whole
   * person to someone else; "not this person". A face nobody has gets a
   * name (a new person) or is attached.
   */
  async function editPerson({ person = null, face = null } = {}) {
    const others = peopleList().filter((other) => other.id !== person?.id);
    const input = h('input', {
      type: 'text',
      value: person ? names.names[person.id] ?? '' : '',
      placeholder: person ? personName(person.id) : t.personName,
      maxlength: 100,
    });
    const hide = h('input', { type: 'checkbox', class: 'switch', checked: person ? names.hidden.includes(person.id) : false });
    let cover = null;
    let attachTo = null;
    let detach = false;
    const whole = h('input', { type: 'checkbox', checked: !face });

    // The faces to choose the one shown from, loaded when asked.
    const covers = h('div', { class: 'cover-picks' });
    const loadCovers = async () => {
      covers.replaceChildren(h('span', { class: 'spinner' }));
      try {
        const { faces: found } = await api('GET', `/people/${person.id}/faces`);
        const current = names.covers?.[person.id] ?? person.cover;
        covers.replaceChildren(
          ...found
            .filter((f) => byId.has(f.file))
            .map((f) => {
              const pick = h(
                'button',
                {
                  type: 'button',
                  class: 'cover-pick' + (f.file === current.file && String(f.b) === String(current.b) ? ' on' : ''),
                  onclick: () => {
                    cover = { file: f.file, b: f.b };
                    covers.querySelectorAll('.cover-pick').forEach((el) => el.classList.toggle('on', el === pick));
                  },
                },
                faceImage(f, 44),
              );
              return pick;
            }),
        );
      } catch (e) {
        covers.replaceChildren(t.error(e.message));
      }
    };

    // Whom to attach to: a face per person.
    const targets = h(
      'div',
      { class: 'attach-picks', hidden: true },
      others.map((other) =>
        h(
          'button',
          {
            type: 'button',
            class: 'attach-pick',
            onclick: (event) => {
              attachTo = attachTo === other.id ? null : other.id;
              targets.querySelectorAll('.attach-pick').forEach((el) => el.classList.toggle('on', el === event.currentTarget && attachTo));
            },
          },
          avatar(other, 36),
          h('span', {}, personName(other.id)),
        ),
      ),
    );
    const attachButton = others.length
      ? h('button', { type: 'button', class: 'btn', onclick: () => (targets.hidden = !targets.hidden) }, t.attach)
      : null;

    const answer = await new Promise((resolve) => {
      let saved = false;
      const form = h(
        'form',
        { method: 'dialog', onsubmit: () => (saved = true) },
        h('div', { class: 'person-head' }, face ? faceImage(face, 72) : avatar(person, 72), h('h2', {}, t.personTitle)),
        h('label', { class: 'field' }, h('span', {}, t.personName), input),
        person ? null : h('p', { class: 'hint', style: 'margin:-4px 0 12px' }, t.newPersonHint),
        person ? h('label', { class: 'toggle-row' }, h('span', {}, t.personHide), hide) : null,
        person
          ? h(
              'details',
              { class: 'cover-section', ontoggle: (event) => event.target.open && !covers.childElementCount && loadCovers() },
              h('summary', {}, t.avatarTitle),
              covers,
            )
          : null,
        attachButton || (face && person)
          ? h(
              'div',
              { class: 'person-tools' },
              attachButton,
              face && person
                ? h(
                    'button',
                    {
                      type: 'button',
                      class: 'btn',
                      onclick: () => {
                        detach = true;
                        saved = true;
                        dialog.close();
                      },
                    },
                    t.notThem,
                  )
                : null,
            )
          : null,
        others.length
          ? h(
              'div',
              { class: 'attach' },
              targets,
              face && person ? h('label', { class: 'check attach-whole' }, whole, h('span', {}, t.attachWhole(person.files.size))) : null,
            )
          : null,
        h(
          'div',
          { class: 'actions' },
          h('button', { class: 'btn', type: 'button', onclick: () => dialog.close() }, t.cancel),
          h('button', { class: 'btn primary', type: 'submit' }, t.save),
        ),
      );
      const dialog = h('dialog', { class: 'person-dialog' }, form);
      // The whole-person box only matters once there is someone to attach to.
      targets.addEventListener('click', () => whole.closest('.attach-whole')?.toggleAttribute('hidden', !attachTo));
      whole.closest('.attach-whole')?.setAttribute('hidden', '');
      dialog.addEventListener('close', () => {
        dialog.remove();
        resolve(saved ? { name: input.value.trim(), hidden: hide.checked } : null);
      });
      document.body.append(dialog);
      dialog.showModal();
      input.focus();
    });
    if (!answer) return;
    try {
      if (detach) {
        ({ names } = await api('PUT', `/faces/${face.file}/${face.i}`, { person: null }));
      } else if (attachTo && face && !(person && whole.checked)) {
        ({ names } = await api('PUT', `/faces/${face.file}/${face.i}`, { person: attachTo }));
      } else if (attachTo) {
        ({ names } = await api('POST', '/people/merge', { from: [person.id], into: attachTo }));
        if (filters.people.delete(person.id)) filters.people.add(attachTo);
      } else if (!person) {
        if (!answer.name) return;
        ({ names } = await api('PUT', `/faces/${face.file}/${face.i}`, { name: answer.name }));
      } else {
        const same = answer.name
          ? others.find((other) => (names.names[other.id] ?? '').toLowerCase() === answer.name.toLowerCase())
          : null;
        if (same && (await ask({ title: t.mergeTitle(answer.name), text: t.mergeText, action: t.merge }))) {
          ({ names } = await api('POST', '/people/merge', { from: [person.id], into: same.id }));
          if (filters.people.delete(person.id)) filters.people.add(same.id);
        } else {
          const change = { name: answer.name, hidden: answer.hidden };
          if (cover) change.cover = cover;
          ({ names } = await api('PUT', `/people/${person.id}`, change));
        }
      }
    } catch (e) {
      problem = t.error(e.message);
    }
    personFilesCache = new Map();
    facesCache.clear();
    viewer?.redrawFaces();
    await reload();
  }

  /** A list of checkboxes, the most common first; long lists fold. */
  function checkSection(name, title, pick) {
    const counts = [...facetCounts(name)].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
    if (!counts.length) return null;
    const limit = 6;
    const open = expanded.has(name);
    const list = open ? counts : counts.slice(0, limit);
    return h(
      'section',
      {},
      h('h3', {}, title),
      h(
        'div',
        { class: 'check-list' },
        list.map(([value, n]) =>
          h(
            'label',
            { class: 'check' },
            h('input', { type: 'checkbox', checked: filters[name].has(value), onchange: () => pick(name, value) }),
            h('span', { class: 'label', title: value }, value),
            h('span', { class: 'n' }, n),
          ),
        ),
      ),
      counts.length > limit
        ? h(
            'button',
            {
              class: 'more',
              onclick: () => {
                if (open) expanded.delete(name);
                else expanded.add(name);
                refresh();
              },
            },
            open ? t.less : t.more(counts.length - limit),
          )
        : null,
    );
  }

  /** Folders as a tree: each with what is in it and below it. */
  function foldersSection(pick) {
    const counts = [...facetCounts('folders')].sort((a, b) => a[0].localeCompare(b[0], intlLocale));
    const limit = 12;
    const open = expanded.has('folders');
    const list = open ? counts : counts.slice(0, limit);
    return h(
      'section',
      {},
      h('h3', {}, t.folders),
      h(
        'div',
        { class: 'check-list' },
        list.map(([path, n]) => {
          const depth = path.split('/').length - 1;
          return h(
            'label',
            { class: 'check', style: `padding-left:${8 + depth * 16}px` },
            h('input', { type: 'checkbox', checked: filters.folders.has(path), onchange: () => pick('folders', path) }),
            h('span', { class: 'label', title: path }, path.split('/').pop()),
            h('span', { class: 'n' }, n),
          );
        }),
      ),
      counts.length > limit
        ? h(
            'button',
            {
              class: 'more',
              onclick: () => {
                if (open) expanded.delete('folders');
                else expanded.add('folders');
                refresh();
              },
            },
            open ? t.less : t.more(counts.length - limit),
          )
        : null,
      h('button', { class: 'btn', style: 'margin-top:10px', onclick: addFolder }, icon('plus'), t.addFolder),
      h('p', { class: 'hint' }, t.removeHint),
    );
  }

  // ── Timeline layout ────────────────────────────────────────────────

  const GAP = 4;
  const SIDE = 16;
  const RIGHT = 40;
  const MONTH_HEIGHT = 56;
  const DAY_HEIGHT = 34;
  /** Between days side by side, and the least width a day's title gets. */
  const DAY_GAP = 16;
  const MIN_DAY_WIDTH = 150;

  /** Blocks of the timeline, top to bottom: month and day titles, rows. */
  let blocks = [];
  let months = [];
  let totalHeight = 0;
  /** Where each shown entry is: its row block, by index in `shown`. */
  let rowOf = [];

  function rowHeight(width) {
    return width < 500 ? 110 : width < 900 ? 160 : 220;
  }

  function aspect(entry) {
    const ratio = entry.width && entry.height ? entry.width / entry.height : 1;
    return Math.min(Math.max(ratio, 0.4), 3);
  }

  /**
   * Justified rows: pictures keep their shape, each full row fills the
   * width exactly; the last row of a day keeps the normal height.
   */
  function justify(items, width, target) {
    const rows = [];
    let row = [];
    let sum = 0;
    for (const item of items) {
      row.push(item);
      sum += item.aspect;
      if (sum * target + GAP * (row.length - 1) >= width) {
        rows.push({ height: (width - GAP * (row.length - 1)) / sum, items: row });
        row = [];
        sum = 0;
      }
    }
    if (row.length) rows.push({ height: target, items: row });
    return rows;
  }

  /**
   * Months, and in them days: a day with many pictures gets full-width
   * justified rows under its title; days with few sit side by side in one
   * row, each with its title above, so a sparse month doesn't become a
   * column of single pictures.
   */
  function computeLayout() {
    const width = Math.max(timeline.clientWidth - SIDE - RIGHT, 100);
    const target = rowHeight(width);
    blocks = [];
    months = [];
    rowOf = new Array(shown.length);
    let y = 8;
    /** The row of small days being filled: its block and how far it is filled. */
    let line = null;
    const closeLine = () => {
      if (!line) return;
      y = line.block.top + line.block.height + GAP + 12;
      line = null;
    };
    let i = 0;
    while (i < shown.length) {
      const day = shown[i]._day;
      const month = shown[i]._month;
      if (!months.length || months[months.length - 1].month !== month) {
        closeLine();
        if (months.length) y += 16;
        const block = { kind: 'month', top: y, height: MONTH_HEIGHT, text: monthTitle(month), month };
        blocks.push(block);
        months.push(block);
        y += MONTH_HEIGHT;
      }
      const items = [];
      while (i < shown.length && shown[i]._day === day) {
        items.push({ entry: shown[i], index: i, aspect: aspect(shown[i]) });
        i++;
      }
      const natural = items.reduce((sum, item) => sum + item.aspect * target, 0) + GAP * (items.length - 1);
      if (natural <= width) {
        const groupWidth = Math.min(Math.max(natural, MIN_DAY_WIDTH), width);
        if (line && line.x + DAY_GAP + groupWidth <= width) {
          line.x += DAY_GAP;
        } else {
          closeLine();
          line = { block: { kind: 'row', top: y, height: DAY_HEIGHT + target, items: [], days: [] }, x: 0 };
          blocks.push(line.block);
        }
        line.block.days.push({ left: SIDE + line.x, width: groupWidth, text: dayTitle(day), day });
        let x = SIDE + line.x;
        for (const item of items) {
          const w = item.aspect * target;
          line.block.items.push({ ...item, left: x, top: DAY_HEIGHT, width: w, height: target });
          rowOf[item.index] = line.block;
          x += w + GAP;
        }
        line.x += groupWidth;
        continue;
      }
      closeLine();
      blocks.push({ kind: 'day', top: y, height: DAY_HEIGHT, text: dayTitle(day), day });
      y += DAY_HEIGHT;
      for (const row of justify(items, width, target)) {
        let x = SIDE;
        const block = { kind: 'row', top: y, height: row.height, items: [], days: [] };
        for (const item of row.items) {
          const w = item.aspect * row.height;
          block.items.push({ ...item, left: x, top: 0, width: w, height: row.height });
          rowOf[item.index] = block;
          x += w + GAP;
        }
        blocks.push(block);
        y += row.height + GAP;
      }
      y += 12;
    }
    closeLine();
    totalHeight = y + 24;
  }

  /** The entry at the top of the view, to keep it there after a change. */
  function anchorItem() {
    if (!blocks.length || !timeline.isConnected) return null;
    const top = timeline.scrollTop;
    const block = blocks[firstBlockAt(top)];
    const row = block && blocks.slice(blocks.indexOf(block)).find((b) => b.items);
    return row ? { id: row.items[0].entry.id, offset: row.top - top } : null;
  }

  /** Tiles of the last layout, kept for the next one: their pictures stay. */
  let reusable = new Map();

  function relayout(anchor) {
    if (!timeline.isConnected) return;
    computeLayout();
    canvas.style.height = `${totalHeight}px`;
    reusable = new Map([...canvas.querySelectorAll('.tile')].map((el) => [el.dataset.key, el]));
    canvas.replaceChildren();
    mounted.clear();
    if (anchor) {
      const index = shown.findIndex((entry) => entry.id === anchor.id);
      timeline.scrollTop = index >= 0 ? rowOf[index].top - anchor.offset : 0;
    } else {
      timeline.scrollTop = 0;
    }
    drawScrubber();
    render();
    reusable.clear();
  }

  /** The first block that reaches below `y`. */
  function firstBlockAt(y) {
    let low = 0;
    let high = blocks.length;
    while (low < high) {
      const mid = (low + high) >> 1;
      if (blocks[mid].top + blocks[mid].height < y) low = mid + 1;
      else high = mid;
    }
    return low;
  }

  /** Blocks on screen and a screen around it, by index. */
  const mounted = new Map();

  function render() {
    const view = timeline.clientHeight;
    const top = timeline.scrollTop - view;
    const bottom = timeline.scrollTop + view * 2;
    const keep = new Set();
    for (let i = firstBlockAt(top); i < blocks.length && blocks[i].top <= bottom; i++) {
      keep.add(i);
      if (!mounted.has(i)) {
        const el = drawBlock(blocks[i]);
        mounted.set(i, el);
        canvas.append(el);
      }
    }
    for (const [i, el] of mounted) {
      if (!keep.has(i)) {
        el.remove();
        mounted.delete(i);
      }
    }
    moveScrubThumb();
  }

  function drawBlock(block) {
    if (block.kind !== 'row') {
      return h(
        'div',
        { class: `group-title ${block.kind}`, style: `top:${block.top}px;height:${block.height}px` },
        block.day ? daySelect(block.day) : null,
        block.text,
      );
    }
    return h(
      'div',
      { style: `position:absolute;left:0;right:0;top:${block.top}px;height:${block.height}px` },
      block.days.map((day) =>
        h(
          'div',
          { class: 'day-inline', title: day.text, style: `left:${day.left}px;width:${day.width}px` },
          daySelect(day.day),
          day.text,
        ),
      ),
      block.items.map(tile),
    );
  }

  /** The circle before a day's title: selects all of the day, or none. */
  function daySelect(day) {
    const on = (dayIds.get(day) ?? []).every((id) => selected.has(id));
    return h(
      'button',
      {
        class: 'day-select' + (on && selected.size ? ' on' : ''),
        'data-day': day,
        title: t.selectDay,
        'aria-label': t.selectDay,
        onclick: () => selectDay(day),
      },
      icon('check'),
    );
  }

  function tile(item) {
    const entry = item.entry;
    const key = `${entry.id}:${entry.etag}:${entry.thumb ? 1 : 0}`;
    const style = `left:${item.left}px;top:${item.top}px;width:${item.width}px;height:${item.height}px`;
    const old = reusable.get(key);
    if (old) {
      reusable.delete(key);
      old.style.cssText = style;
      old.dataset.index = item.index;
      old.classList.toggle('selected', selected.has(entry.id));
      return old;
    }
    const index = () => Number(el.dataset.index);
    // While something is selected, a click selects too.
    const open = (event) => {
      if (selected.size || event.shiftKey || event.ctrlKey || event.metaKey) toggleSelect(index(), event.shiftKey);
      else openViewer(index());
    };
    const el = h('div', {
      class: 'tile',
      role: 'button',
      tabindex: 0,
      'aria-label': entry.name,
      'data-key': key,
      'data-id': entry.id,
      'data-index': item.index,
      style,
      onclick: open,
      onkeydown: (event) => {
        if (viewer) return;
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault();
          open(event);
        }
      },
    });
    el.classList.toggle('selected', selected.has(entry.id));
    if (entry.thumb) {
      const img = h('img', { alt: '', decoding: 'async', draggable: 'false' });
      img.addEventListener('load', () => img.classList.add('loaded'));
      link('thumb', entry).then((url) => {
        if (url) img.src = url;
      });
      el.append(img);
    } else {
      el.append(h('div', { class: 'placeholder' }, icon(entry.error ? 'alert' : entry.video ? 'video' : 'image'), entry._format));
    }
    if (entry.video) {
      el.append(
        h('span', { class: 'mark top-right' }, entry.duration ? duration(entry.duration) : null, icon('play')),
      );
    }
    el.append(
      h(
        'button',
        {
          class: 'select-toggle',
          title: t.select,
          'aria-label': t.select,
          tabindex: -1,
          onclick: (event) => {
            event.stopPropagation();
            toggleSelect(index(), event.shiftKey);
          },
        },
        icon('check'),
      ),
    );
    const favorite = favorites.has(entry.id);
    el.append(
      h(
        'button',
        {
          class: 'fav-toggle' + (favorite ? ' on' : ''),
          title: favorite ? t.unfavorite : t.favorite,
          'aria-label': favorite ? t.unfavorite : t.favorite,
          onclick: (event) => {
            event.stopPropagation();
            toggleFavorite(entry);
          },
        },
        icon('heart'),
      ),
    );
    return el;
  }

  let renderQueued = false;
  timeline.addEventListener('scroll', () => {
    if (renderQueued) return;
    renderQueued = true;
    requestAnimationFrame(() => {
      renderQueued = false;
      render();
      showScrubLabel(monthAt(timeline.scrollTop));
    });
  });

  let lastWidth = 0;
  new ResizeObserver(() => {
    if (!timeline.isConnected || timeline.clientWidth === lastWidth) return;
    lastWidth = timeline.clientWidth;
    relayout(anchorItem());
  }).observe(timeline);

  // ── Scrubber: years down the right side ────────────────────────────

  function scrubY(top) {
    const height = scrubber.clientHeight;
    return totalHeight ? (top / totalHeight) * height : 0;
  }

  function drawScrubber() {
    scrubber.replaceChildren();
    let lastYear = null;
    let lastLabel = -100;
    let lastDot = -100;
    for (const month of months) {
      const y = scrubY(month.top);
      const year = month.month.slice(0, 4);
      if (year !== lastYear && y - lastLabel >= 18) {
        scrubber.append(h('span', { class: 'year', style: `top:${y}px` }, year));
        lastLabel = y;
      } else if (y - lastDot >= 6 && y - lastLabel >= 10) {
        scrubber.append(h('span', { class: 'dot', style: `top:${y}px` }));
        lastDot = y;
      }
      lastYear = year;
    }
    scrubber.append(h('span', { class: 'thumb-line' }));
  }

  function moveScrubThumb() {
    const line = scrubber.querySelector('.thumb-line');
    if (line) line.style.top = `${scrubY(timeline.scrollTop)}px`;
  }

  function monthAt(y) {
    let found = months[0];
    for (const month of months) {
      if (month.top > y + 1) break;
      found = month;
    }
    return found;
  }

  let labelTimer = null;
  function showScrubLabel(month, y) {
    if (!month) return;
    scrubLabel.textContent = month.text;
    scrubLabel.style.top = `${8 + (y ?? scrubY(timeline.scrollTop))}px`;
    scrubLabel.classList.add('show');
    clearTimeout(labelTimer);
    labelTimer = setTimeout(() => scrubLabel.classList.remove('show'), 900);
  }

  function scrubTo(event) {
    const rect = scrubber.getBoundingClientRect();
    const y = Math.min(Math.max(event.clientY - rect.top, 0), rect.height);
    const top = (y / rect.height) * totalHeight;
    timeline.scrollTop = top;
    showScrubLabel(monthAt(top), y);
  }

  scrubber.addEventListener('pointerdown', (event) => {
    scrubber.setPointerCapture(event.pointerId);
    scrubTo(event);
  });
  scrubber.addEventListener('pointermove', (event) => {
    if (scrubber.hasPointerCapture(event.pointerId)) scrubTo(event);
  });

  // ── Actions ────────────────────────────────────────────────────────

  async function addFolder() {
    try {
      // Writing is for uploading and deleting; the gallery never changes files.
      await rustcloud.requestAccess({ folder: t.folderName, write: true });
    } catch {
      if (!folders.length) {
        notice = t.denied;
        refresh();
      }
      return;
    }
    notice = '';
    await reload();
  }

  async function reload() {
    try {
      await loadState();
      problem = '';
    } catch (e) {
      problem = t.error(e.message);
    }
    refresh();
    schedulePoll();
  }

  /** Favourites change one request at a time, in the order clicked. */
  let favoriteChain = Promise.resolve();
  function toggleFavorite(entry) {
    const on = !favorites.has(entry.id);
    if (on) favorites.add(entry.id);
    else favorites.delete(entry.id);
    updateFavoriteMarks(entry);
    favoriteChain = favoriteChain.then(async () => {
      try {
        const result = await api(on ? 'PUT' : 'DELETE', `/favorites/${entry.id}`);
        favorites = new Set(result.favorites);
      } catch (e) {
        problem = t.error(e.message);
        refresh();
      }
    });
    if (filters.favorites) refresh();
  }

  /** Updates the heart of one entry where it is drawn. */
  function updateFavoriteMarks(entry) {
    const on = favorites.has(entry.id);
    for (const button of canvas.querySelectorAll(`.tile[data-id="${entry.id}"] .fav-toggle`)) {
      button.classList.toggle('on', on);
      button.title = on ? t.unfavorite : t.favorite;
      button.setAttribute('aria-label', button.title);
    }
    if (viewer?.entry.id === entry.id) viewer.drawBar();
  }

  /** Asks the user in a <dialog> (confirm() doesn't work in the sandbox); true for yes. */
  function ask({ title, text, action, danger = false }) {
    return new Promise((resolve) => {
      let answer = false;
      const yes = h('button', { class: `btn ${danger ? 'danger' : 'primary'}`, onclick: () => ((answer = true), dialog.close()) }, action);
      const dialog = h(
        'dialog',
        {},
        h('h2', {}, title),
        h('p', {}, text),
        h('div', { class: 'actions' }, h('button', { class: 'btn', onclick: () => dialog.close() }, t.cancel), yes),
      );
      dialog.addEventListener('close', () => {
        dialog.remove();
        resolve(answer);
      });
      document.body.append(dialog);
      dialog.showModal();
      yes.focus();
    });
  }

  async function confirmRebuild() {
    if (!(await ask({ title: t.rebuildTitle, text: t.rebuildText, action: t.rebuild }))) return;
    try {
      await api('POST', '/rebuild');
      notice = t.rebuildStarted;
    } catch (e) {
      problem = t.error(e.message);
    }
    await reload();
  }

  /**
   * Deletes a file after asking; true if it is gone. A folder given for
   * reading only is offered to be given again with writing.
   */
  async function deleteEntry(entry) {
    if (!(await ask({ title: t.deleteTitle(entry.name), text: t.deleteText, action: t.deleteButton, danger: true }))) {
      return false;
    }
    for (;;) {
      try {
        favorites = new Set((await api('DELETE', `/files/${entry.id}`)).favorites);
        break;
      } catch (e) {
        if (e.code !== 'read_only') {
          problem = t.error(e.message);
          refresh();
          return false;
        }
        const folder = folders.find((f) => f.id === entry.root)?.name ?? entry.folder.split('/')[0];
        if (!(await ask({ title: t.needWriteTitle, text: t.needWriteText(folder), action: t.allow }))) return false;
        try {
          await rustcloud.requestAccess({ folder, write: true });
        } catch {
          return false;
        }
      }
    }
    deletedIds.add(entry.id);
    library = library.filter((e) => e.id !== entry.id);
    refresh();
    // The job that drops it from the index is running: follow it.
    reload();
    return true;
  }

  // ── Selection ──────────────────────────────────────────────────────

  /** Selects or unselects one entry; with `range`, all from the last one clicked. */
  function toggleSelect(index, range) {
    const entry = shown[index];
    if (!entry) return;
    if (range && selectAnchor !== null && shown[selectAnchor]) {
      const [from, to] = selectAnchor < index ? [selectAnchor, index] : [index, selectAnchor];
      for (let i = from; i <= to; i++) selected.add(shown[i].id);
    } else if (selected.has(entry.id)) {
      selected.delete(entry.id);
    } else {
      selected.add(entry.id);
    }
    selectAnchor = index;
    selectionChanged();
  }

  function selectDay(day) {
    const ids = dayIds.get(day) ?? [];
    const all = ids.every((id) => selected.has(id));
    for (const id of ids) {
      if (all) selected.delete(id);
      else selected.add(id);
    }
    selectionChanged();
  }

  function selectAll() {
    for (const entry of shown) selected.add(entry.id);
    selectionChanged();
  }

  function clearSelection() {
    selected.clear();
    selectAnchor = null;
    selectionStatus = '';
    selectionChanged();
  }

  /** Marks the drawn tiles and days, and swaps the top bar. */
  function selectionChanged() {
    if (!selected.size) selectAnchor = null;
    topEl = swap(topEl, topbar());
    canvas.classList.toggle('selecting', selected.size > 0);
    for (const el of canvas.querySelectorAll('.tile')) el.classList.toggle('selected', selected.has(el.dataset.id));
    for (const el of canvas.querySelectorAll('.day-select')) {
      const ids = dayIds.get(el.dataset.day) ?? [];
      el.classList.toggle('on', selected.size > 0 && ids.every((id) => selected.has(id)));
    }
  }

  function selectedEntries() {
    return shown.filter((entry) => selected.has(entry.id));
  }

  function selectionBar() {
    const entries = selectedEntries();
    const allFavorite = entries.every((entry) => favorites.has(entry.id));
    const busy = Boolean(selectionStatus);
    return h(
      'header',
      { class: 'topbar selecting' },
      iconButton('close', t.clearSelection, clearSelection),
      h('h1', {}, t.selected(selected.size)),
      busy ? h('span', { class: 'progress-chip' }, h('span', { class: 'spinner' }), selectionStatus) : null,
      h('div', { class: 'spacer' }),
      iconButton('selectAll', t.selectAll, selectAll),
      iconButton('heart', allFavorite ? t.unfavorite : t.favorite, favoriteSelected, {
        class: 'icon-btn' + (allFavorite ? ' on' : ''),
      }),
      iconButton('download', t.download, downloadSelected, { disabled: busy }),
      iconButton('trash', t.delete, deleteSelected, { disabled: busy }),
    );
  }

  /** Favourites all of the selection, or, when all are, none of it. */
  async function favoriteSelected() {
    const ids = selectedEntries().map((entry) => entry.id);
    const on = !ids.every((id) => favorites.has(id));
    for (const id of ids) {
      if (on) favorites.add(id);
      else favorites.delete(id);
    }
    refreshFavoriteMarks();
    try {
      for (let start = 0; start < ids.length; start += 1000) {
        const result = await api('POST', '/favorites', { ids: ids.slice(start, start + 1000), on });
        favorites = new Set(result.favorites);
      }
    } catch (e) {
      problem = t.error(e.message);
    }
    refresh();
    refreshFavoriteMarks();
  }

  function refreshFavoriteMarks() {
    for (const button of canvas.querySelectorAll('.tile .fav-toggle')) {
      const on = favorites.has(button.closest('.tile').dataset.id);
      button.classList.toggle('on', on);
      button.title = on ? t.unfavorite : t.favorite;
      button.setAttribute('aria-label', button.title);
    }
    if (selected.size) topEl = swap(topEl, topbar());
  }

  /** Saves the original of an entry: through a blob, as the link is of another origin. */
  async function downloadEntry(entry) {
    const url = await link('original', entry);
    if (!url) throw new Error('not found');
    const response = await fetch(url);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const blobUrl = URL.createObjectURL(await response.blob());
    const anchor = h('a', { href: blobUrl, download: entry.name });
    document.body.append(anchor);
    anchor.click();
    anchor.remove();
    setTimeout(() => URL.revokeObjectURL(blobUrl), 60_000);
  }

  /** One file after another (the browser asks once whether to allow several). */
  async function downloadSelected() {
    const entries = selectedEntries();
    try {
      for (const [i, entry] of entries.entries()) {
        selectionStatus = t.downloadingMany(i + 1, entries.length);
        if (selected.size) topEl = swap(topEl, topbar());
        await downloadEntry(entry);
        await new Promise((resolve) => setTimeout(resolve, 300));
      }
    } catch (e) {
      problem = t.error(e.message);
    }
    selectionStatus = '';
    refresh();
  }

  /** Moves the selection to the trash after asking; read-only folders are offered to be given again. */
  async function deleteSelected() {
    const entries = selectedEntries();
    if (!entries.length) return;
    const confirmed = await ask({
      title: t.deleteManyTitle(entries.length),
      text: t.deleteManyText,
      action: t.deleteButton,
      danger: true,
    });
    if (!confirmed) return;
    let pending = entries.map((entry) => entry.id);
    const deleted = [];
    let failed = 0;
    let asked = false;
    try {
      while (pending.length) {
        const readOnly = [];
        for (let start = 0; start < pending.length; start += 1000) {
          const result = await api('POST', '/files/delete', { ids: pending.slice(start, start + 1000) });
          deleted.push(...result.deleted);
          readOnly.push(...result.readOnly);
          failed += result.failed.length;
          favorites = new Set(result.favorites);
        }
        pending = [];
        if (!readOnly.length || asked) {
          failed += readOnly.length;
          break;
        }
        // Once: the folders given for reading only, given again with writing.
        asked = true;
        const names = [
          ...new Set(
            entries
              .filter((entry) => readOnly.includes(entry.id))
              .map((entry) => folders.find((f) => f.id === entry.root)?.name ?? entry.folder.split('/')[0]),
          ),
        ];
        if (!(await ask({ title: t.needWriteTitle, text: t.needWriteText(names.join(', ')), action: t.allow }))) {
          failed += readOnly.length;
          break;
        }
        try {
          for (const folder of names) await rustcloud.requestAccess({ folder, write: true });
          pending = readOnly;
        } catch {
          failed += readOnly.length;
        }
      }
    } catch (e) {
      problem = t.error(e.message);
    }
    for (const id of deleted) {
      deletedIds.add(id);
      selected.delete(id);
    }
    const gone = new Set(deleted);
    library = library.filter((entry) => !gone.has(entry.id));
    if (failed) problem = t.deleteFailed(failed);
    selectionStatus = '';
    refresh();
    selectionChanged();
    reload();
  }

  // Esc, Ctrl+A and Delete for the selection; the viewer and dialogs have their own keys.
  document.addEventListener('keydown', (event) => {
    if (viewer || document.querySelector('dialog[open]')) return;
    if (event.target.closest?.('input, textarea, select')) return;
    if (event.key === 'Escape' && selected.size) {
      event.preventDefault();
      clearSelection();
    } else if ((event.ctrlKey || event.metaKey) && (event.key === 'a' || event.key === 'ф') && shown.length) {
      event.preventDefault();
      selectAll();
    } else if (event.key === 'Delete' && selected.size && !selectionStatus) {
      event.preventDefault();
      deleteSelected();
    }
  });

  // ── Uploads ────────────────────────────────────────────────────────

  // What the browser may not name a type for (RAW, HEIC, some videos).
  const MEDIA_NAME =
    /\.(jpe?g|png|webp|gif|avif|bmp|heic|heif|tiff?|cr2|cr3|nef|nrw|arw|srf|sr2|dng|raf|orf|rw2|pef|srw|x3f|mp4|m4v|mov|mkv|webm|avi|mts|m2ts|3gp|wmv|mpe?g)$/i;
  const UPLOAD_ACCEPT = 'image/*,video/*,.heic,.heif,.cr2,.cr3,.nef,.arw,.dng,.raf,.orf,.rw2,.mkv,.mts';
  // Files per request for links: a link must be used within 15 minutes.
  const LINKS_AT_ONCE = 30;
  const UPLOADS_AT_ONCE = 3;

  function isMediaFile(file) {
    if (file.name.startsWith('.') || file.type === 'image/svg+xml') return false;
    return file.type.startsWith('image/') || file.type.startsWith('video/') || MEDIA_NAME.test(file.name);
  }

  function pickFiles() {
    const input = h('input', { type: 'file', multiple: true, accept: UPLOAD_ACCEPT });
    input.addEventListener('change', () => upload([...input.files].map((file) => ({ file, dir: '' }))));
    input.click();
  }

  /**
   * What a drop carries, taken while its event lasts (the lists are emptied
   * after it): the files, and the folders' entries when folders were dropped.
   */
  function takeDrop(dataTransfer) {
    const dirs = [];
    for (const item of dataTransfer.items ?? []) {
      try {
        const entry = item.webkitGetAsEntry?.();
        if (entry?.isDirectory) dirs.push(entry);
      } catch {
        // No entries here: the files are enough.
      }
    }
    const dirNames = new Set(dirs.map((dir) => dir.name));
    const files = [...dataTransfer.files].filter((file) => !(dirNames.has(file.name) && !file.type));
    return { files, dirs };
  }

  /**
   * The files of a drop ({file, dir}), the ones inside dropped folders too.
   * A sandboxed page may not be let read folders: then `foldersRefused`.
   */
  async function droppedFiles({ files, dirs }) {
    const found = files.map((file) => ({ file, dir: '' }));
    if (!dirs.length) return { found, foldersRefused: false };
    const inFolders = [];
    const walk = async (entry, dir) => {
      if (entry.isFile) {
        inFolders.push({ file: await new Promise((resolve, reject) => entry.file(resolve, reject)), dir });
        return;
      }
      const reader = entry.createReader();
      const path = dir ? `${dir}/${entry.name}` : entry.name;
      // readEntries gives a part at a time, until an empty one.
      for (;;) {
        const part = await new Promise((resolve, reject) => reader.readEntries(resolve, reject));
        if (!part.length) break;
        for (const child of part) await walk(child, path);
      }
    };
    try {
      for (const entry of dirs) await walk(entry, '');
    } catch {
      return { found, foldersRefused: true };
    }
    return { found: found.concat(inFolders), foldersRefused: false };
  }

  /** The granted folder to upload into: asked when there are several. */
  async function uploadFolder() {
    if (!folders.length) await addFolder();
    if (folders.length < 2) return folders[0] ?? null;
    const chosen = folders.some((f) => f.id === uploadFolderId) ? uploadFolderId : folders[0].id;
    const options = folders.map((folder) =>
      h(
        'label',
        { class: 'pick' },
        h('input', { type: 'radio', name: 'upload-folder', value: folder.id, checked: folder.id === chosen }),
        icon('folder'),
        folder.name,
      ),
    );
    return new Promise((resolve) => {
      let answer = null;
      const yes = h('button', { class: 'btn primary', type: 'submit' }, t.uploadButton);
      const cancel = h('button', { class: 'btn', type: 'button', onclick: () => dialog.close() }, t.cancel);
      const form = h(
        'form',
        { method: 'dialog' },
        h('h2', {}, t.uploadTitle),
        h('div', { class: 'picks' }, options),
        h('div', { class: 'actions' }, cancel, yes),
      );
      form.addEventListener('submit', () => {
        const id = form.querySelector('input:checked')?.value;
        answer = folders.find((f) => f.id === id) ?? null;
        if (answer && id !== uploadFolderId) {
          uploadFolderId = id;
          api('PUT', `/upload-folder/${id}`).catch(() => {});
        }
      });
      const dialog = h('dialog', {}, form);
      dialog.addEventListener('close', () => {
        dialog.remove();
        resolve(answer);
      });
      document.body.append(dialog);
      dialog.showModal();
      yes.focus();
    });
  }

  /** The uploads under way, and those done since the panel was last closed. */
  const uploads = { total: 0, done: 0, bytes: 0, sent: 0, failed: [], skipped: 0, running: false };
  const uploadsEl = h('div', { class: 'uploads', hidden: true, role: 'status' });
  document.body.append(uploadsEl);
  let uploadsHideTimer = null;

  function clearUploads() {
    Object.assign(uploads, { total: 0, done: 0, bytes: 0, sent: 0, failed: [], skipped: 0 });
    drawUploads();
  }

  function drawUploads() {
    clearTimeout(uploadsHideTimer);
    const u = uploads;
    if (!u.total && !u.skipped && !u.failed.length) {
      uploadsEl.hidden = true;
      return;
    }
    const head = u.running
      ? [h('span', { class: 'spinner' }), h('b', {}, t.uploading(Math.min(u.done + 1, u.total), u.total))]
      : [icon(u.failed.length ? 'alert' : 'check'), h('b', {}, t.uploaded(u.done - u.failed.length))];
    const parts = [
      h(
        'div',
        { class: 'uploads-head' },
        ...head,
        h('div', { class: 'spacer' }),
        u.running ? null : iconButton('close', t.close, clearUploads),
      ),
      u.running ? h('div', { class: 'uploads-bar', style: `--done:${u.bytes ? (u.sent / u.bytes) * 100 : 0}%` }) : null,
      u.skipped ? h('div', { class: 'uploads-note' }, t.uploadSkipped(u.skipped)) : null,
      u.failed.length
        ? h(
            'ul',
            { class: 'uploads-failed' },
            u.failed.map(({ name, why }) => h('li', {}, h('span', { class: 'name' }, name), ` — ${why}`)),
          )
        : null,
    ];
    uploadsEl.replaceChildren(...parts.filter(Boolean));
    uploadsEl.hidden = false;
    // A clean finish goes away by itself.
    if (!u.running && !u.failed.length && !u.skipped) uploadsHideTimer = setTimeout(clearUploads, 4000);
  }

  /** Why an upload failed, in words. */
  function uploadError(status, code, message) {
    if (code === 'FILE_TOO_LARGE') return t.tooLarge;
    if (code === 'QUOTA_EXCEEDED') return t.noSpace;
    if (code === 'NAME_TAKEN' || code === 'conflict') return t.nameTaken;
    if (code === 'READ_ONLY') return t.readOnly;
    if (code === 'LINK_NOT_FOUND') return t.linkGone;
    if (status === 0) return t.networkError;
    return message || `HTTP ${status}`;
  }

  /** Sends a file to its link; progress goes to `onProgress(bytes sent)`. */
  function send(url, file, onProgress) {
    return new Promise((resolve) => {
      // XMLHttpRequest: fetch() tells nothing of an upload's progress.
      const xhr = new XMLHttpRequest();
      xhr.open('POST', url);
      xhr.upload.onprogress = (event) => onProgress(event.loaded);
      xhr.onload = () => {
        let error = {};
        try {
          error = JSON.parse(xhr.responseText).error ?? {};
        } catch {}
        resolve({ ok: xhr.status === 201, status: xhr.status, code: error.code, message: error.message });
      };
      xhr.onerror = () => resolve({ ok: false, status: 0 });
      xhr.send(file);
    });
  }

  /** Links for a part of the files; a read-only folder is offered to be given again. Null if not. */
  async function uploadLinks(folder, part) {
    const items = part.map(({ file, dir }) => ({ dir, name: file.name }));
    for (;;) {
      try {
        return (await api('POST', '/uploads', { folder: folder.id, items })).items;
      } catch (e) {
        if (e.code !== 'read_only') throw e;
        if (!(await ask({ title: t.needWriteTitle, text: t.needWriteUploadText(folder.name), action: t.allow }))) {
          return null;
        }
        try {
          await rustcloud.requestAccess({ folder: folder.name, write: true });
        } catch {
          return null;
        }
      }
    }
  }

  /**
   * Uploads the photos and videos of `list` ({file, dir}) into a granted
   * folder; `note` is shown meanwhile (what the drop couldn't give).
   */
  async function upload(list, note = '') {
    if (uploads.running) return;
    const media = list.filter(({ file }) => isMediaFile(file));
    if (!media.length) {
      notice = note || t.nothingToUpload;
      refresh();
      return;
    }
    const folder = await uploadFolder();
    if (!folder) return;
    notice = note;
    const u = uploads;
    Object.assign(u, { total: media.length, done: 0, sent: 0, failed: [], skipped: list.length - media.length });
    u.bytes = media.reduce((sum, { file }) => sum + file.size, 0);
    u.running = true;
    drawUploads();
    const fail = (file, why) => {
      u.failed.push({ name: file.name, why });
      u.done++;
      u.sent += file.size;
    };
    try {
      for (let start = 0; start < media.length; start += LINKS_AT_ONCE) {
        const part = media.slice(start, start + LINKS_AT_ONCE);
        let answers;
        try {
          answers = await uploadLinks(folder, part);
        } catch (e) {
          answers = e;
        }
        if (!Array.isArray(answers)) {
          // No links: the rest isn't sent.
          const why = answers ? answers.message : t.readOnly;
          for (const { file } of media.slice(start)) fail(file, why);
          break;
        }
        const queue = part.map((item, i) => ({ ...item, answer: answers[i] }));
        const worker = async () => {
          for (let next = queue.shift(); next; next = queue.shift()) {
            const { file, answer } = next;
            if (!answer.url) {
              fail(file, uploadError(400, answer.code, answer.error));
              drawUploads();
              continue;
            }
            const before = u.sent;
            const progress = (sent) => {
              u.sent = before + sent;
              drawUploads();
            };
            let result = await send(answer.url, file, progress);
            // A broken connection: the same link may be tried once more.
            if (result.status === 0) result = await send(answer.url, file, progress);
            u.sent = before;
            if (result.ok) {
              u.done++;
              u.sent += file.size;
            } else {
              fail(file, uploadError(result.status, result.code, result.message));
            }
            drawUploads();
          }
        };
        await Promise.all(Array.from({ length: UPLOADS_AT_ONCE }, worker));
        // What came so far appears while the rest goes.
        reload();
      }
    } finally {
      u.running = false;
      drawUploads();
      reload();
    }
  }

  // Files dropped anywhere on the page are uploaded. Pictures dragged inside
  // the page are no files to upload.
  const dropEl = h('div', { class: 'drop', hidden: true });
  document.body.append(dropEl);
  let draggingInside = false;
  let dragDepth = 0;
  const carriesFiles = (event) => !draggingInside && [...(event.dataTransfer?.types ?? [])].includes('Files');
  document.addEventListener('dragstart', () => (draggingInside = true));
  document.addEventListener('dragend', () => (draggingInside = false));
  document.addEventListener('dragenter', (event) => {
    if (!carriesFiles(event)) return;
    event.preventDefault();
    if (dragDepth++ > 0) return;
    const where = folders.length === 1 ? t.dropTo(folders[0].name) : folders.length ? t.dropChoose : '';
    dropEl.replaceChildren(
      h('div', { class: 'drop-card' }, icon('upload'), h('b', {}, t.dropHint), where ? h('span', {}, where) : null),
    );
    dropEl.hidden = false;
  });
  document.addEventListener('dragover', (event) => {
    if (!carriesFiles(event)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'copy';
  });
  document.addEventListener('dragleave', (event) => {
    if (!carriesFiles(event) || --dragDepth > 0) return;
    dragDepth = 0;
    dropEl.hidden = true;
  });
  document.addEventListener('drop', async (event) => {
    if (!carriesFiles(event)) return;
    event.preventDefault();
    dragDepth = 0;
    dropEl.hidden = true;
    const dropped = takeDrop(event.dataTransfer);
    try {
      const { found, foldersRefused } = await droppedFiles(dropped);
      await upload(found, foldersRefused ? t.foldersRefused : '');
    } catch (e) {
      notice = t.dropFailed(e?.message ?? String(e));
      refresh();
    }
  });
  addEventListener('beforeunload', (event) => {
    if (uploads.running) event.preventDefault();
  });

  // ── Viewer ─────────────────────────────────────────────────────────

  /** The open viewer, or null. */
  let viewer = null;

  function openViewer(index) {
    viewer?.close(true);
    viewer = new Viewer(index);
  }

  /** Video codecs as the browser asks for them. */
  const CODECS = { h264: 'avc1.640028', hevc: 'hvc1.1.6.L120.90', av1: 'av01.0.08M.08', vp9: 'vp09.00.40.08', vp8: 'vp8' };

  /** Whether the browser plays the file as it is, by its link. */
  function playsAsIs(entry) {
    const codec = CODECS[entry.codec];
    if (!codec) return false;
    const probe = document.createElement('video');
    if (['mp4', 'mov', 'm4v', '3gp'].includes(entry.container) || /mp4|quicktime/.test(entry.mime)) {
      return probe.canPlayType(`video/mp4; codecs="${codec}"`) !== '';
    }
    if (entry.container === 'webm' || entry.mime === 'video/webm') {
      return probe.canPlayType(`video/webm; codecs="${codec}"`) !== '';
    }
    return false;
  }

  /** Whether the browser decodes the codec, so repacking is enough. */
  function decodes(entry) {
    const codec = CODECS[entry.codec];
    return Boolean(codec && window.MediaSource?.isTypeSupported(`video/mp4; codecs="${codec}"`));
  }

  let hlsScript = null;
  function loadHls() {
    hlsScript ??= new Promise((resolve, reject) => {
      const script = h('script', { src: 'vendor/hls.min.js' });
      script.onload = () => resolve(window.Hls);
      script.onerror = () => reject(new Error('hls.js'));
      document.head.append(script);
    });
    return hlsScript;
  }

  /** What the player keeps from one video to the next. */
  const player = { volume: 1, muted: false, rate: 1 };
  const RATES = [1, 1.25, 1.5, 2, 0.5];

  /**
   * Our own controls for a video. The browser's would sit under the viewer's
   * bars, its full-screen button can't work in the sandbox, and its keys
   * would fight the viewer's.
   */
  class VideoControls {
    /** `done` is called after each use, to give the keys back to the viewer. */
    constructor(video, entry, done) {
      this.video = video;
      this.entry = entry;
      video.volume = player.volume;
      video.muted = player.muted;
      video.defaultPlaybackRate = video.playbackRate = player.rate;

      const button = (name, label, onclick) =>
        iconButton(name, label, () => {
          onclick();
          done();
        });
      this.playButton = button('play', t.play, () => this.toggle());
      this.time = h('span', { class: 'vtime' });
      this.seek = h('input', { type: 'range', class: 'vseek', min: 0, max: 1000, step: 1, value: 0, 'aria-label': t.seek, title: t.seek });
      this.muteButton = button('volume', t.mute, () => this.toggleMute());
      this.volume = h('input', { type: 'range', class: 'vvolume', min: 0, max: 100, step: 1, 'aria-label': t.volume, title: t.volume });
      this.rateButton = h('button', { class: 'vrate', title: t.speed, onclick: () => (this.nextRate(), done()) });
      this.root = h(
        'div',
        { class: 'vcontrols' },
        this.seek,
        h('div', { class: 'vrow' }, this.playButton, this.time, h('div', { class: 'spacer' }), this.muteButton, this.volume, this.rateButton),
      );
      this.big = h('button', { class: 'vbig', 'aria-label': t.play, onclick: () => (this.toggle(), done()) }, icon('play'));
      this.waiting = h('div', { class: 'vwait', hidden: true }, h('div', { class: 'spinner' }));

      // Dragging the position shows the time; letting go seeks (once: HLS
      // pieces are made on request).
      this.seek.addEventListener('input', () => {
        this.dragging = true;
        this.drawTime(this.seekTarget());
      });
      this.seek.addEventListener('change', () => {
        this.dragging = false;
        video.currentTime = this.seekTarget();
        done();
      });
      this.volume.addEventListener('input', () => {
        video.volume = Number(this.volume.value) / 100;
        video.muted = video.volume === 0;
      });
      this.volume.addEventListener('change', done);

      video.addEventListener('click', () => (this.toggle(), done()));
      for (const name of ['play', 'pause', 'ended']) video.addEventListener(name, () => this.drawState());
      for (const name of ['timeupdate', 'progress', 'durationchange', 'loadedmetadata']) {
        video.addEventListener(name, () => this.drawPosition());
      }
      video.addEventListener('volumechange', () => {
        player.volume = video.volume;
        player.muted = video.muted;
        this.drawVolume();
      });
      video.addEventListener('ratechange', () => {
        player.rate = video.playbackRate;
        this.drawRate();
      });
      video.addEventListener('waiting', () => (this.waiting.hidden = false));
      for (const name of ['playing', 'canplay', 'error']) video.addEventListener(name, () => (this.waiting.hidden = true));
      this.drawState();
      this.drawPosition();
      this.drawVolume();
      this.drawRate();
    }

    /** Seconds; HLS may not know its length at first, the index does. */
    get duration() {
      const d = this.video.duration;
      return Number.isFinite(d) && d > 0 ? d : (this.entry.duration || 0) / 1000;
    }

    seekTarget() {
      return (Number(this.seek.value) / 1000) * this.duration;
    }

    toggle() {
      if (this.video.paused || this.video.ended) this.video.play().catch(() => this.drawState());
      else this.video.pause();
    }

    toggleMute() {
      // Unmuting at volume 0 would stay silent.
      if (this.video.muted && this.video.volume === 0) this.video.volume = 0.5;
      this.video.muted = !this.video.muted;
    }

    changeVolume(step) {
      this.video.muted = false;
      this.video.volume = Math.min(1, Math.max(0, Math.round((this.video.volume + step) * 10) / 10));
    }

    seekBy(seconds) {
      this.video.currentTime = Math.min(Math.max(this.video.currentTime + seconds, 0), this.duration || Infinity);
    }

    nextRate() {
      this.video.playbackRate = RATES[(RATES.indexOf(this.video.playbackRate) + 1) % RATES.length] ?? 1;
    }

    drawState() {
      const paused = this.video.paused || this.video.ended;
      this.playButton.replaceChildren(icon(paused ? 'play' : 'pause'));
      this.playButton.title = paused ? t.play : t.pause;
      this.playButton.setAttribute('aria-label', this.playButton.title);
      this.big.hidden = !paused;
    }

    drawPosition() {
      const duration = this.duration;
      const now = this.video.currentTime;
      const buffered = this.video.buffered;
      let loaded = 0;
      for (let i = 0; i < buffered.length; i++) {
        if (buffered.start(i) <= now + 0.5) loaded = Math.max(loaded, buffered.end(i));
      }
      const percent = (seconds) => (duration ? Math.min(100, (seconds / duration) * 100) : 0);
      this.seek.style.setProperty('--buffered', `${percent(loaded)}%`);
      if (!this.dragging) {
        this.seek.value = String(Math.round(percent(now) * 10));
        this.seek.style.setProperty('--played', `${percent(now)}%`);
        this.drawTime(now);
      }
    }

    drawTime(seconds) {
      const total = this.duration;
      this.time.textContent = `${durationText(seconds)} / ${total ? durationText(total) : '–:––'}`;
    }

    drawVolume() {
      const silent = this.video.muted || this.video.volume === 0;
      this.muteButton.replaceChildren(icon(silent ? 'muted' : 'volume'));
      this.muteButton.title = silent ? t.unmute : t.mute;
      this.muteButton.setAttribute('aria-label', this.muteButton.title);
      const level = silent ? 0 : Math.round(this.video.volume * 100);
      this.volume.value = String(level);
      this.volume.style.setProperty('--level', `${level}%`);
    }

    drawRate() {
      this.rateButton.textContent = `${this.video.playbackRate}×`;
    }

    remove() {
      this.root.remove();
      this.big.remove();
      this.waiting.remove();
    }
  }

  const PHOTOS_SHOWN = ['image/jpeg', 'image/png', 'image/webp', 'image/gif', 'image/avif', 'image/bmp'];

  class Viewer {
    constructor(index) {
      // Its own list: the timeline may change behind it while jobs run.
      this.list = shown;
      this.index = index;
      this.infoOpen = innerWidth > 1100;
      this.view = { z: 1, x: 0, y: 0 };
      this.pointers = new Map();
      this.build();
      this.show(index, { from: this.tileRect(this.entry) });
      requestAnimationFrame(() => this.root.classList.add('open'));
    }

    get entry() {
      return this.list[this.index];
    }

    focus() {
      this.root.focus({ preventScroll: true });
    }

    build() {
      this.blur = h('div', { class: 'blur' });
      this.stage = h('div', { class: 'stage' });
      this.bar = h('div', { class: 'bar' });
      this.status = h('div', { class: 'status' });
      this.strip = h('div', { class: 'strip' });
      this.prevButton = h('button', { class: 'nav prev', 'aria-label': t.prev, onclick: () => this.go(-1) }, icon('left'));
      this.nextButton = h('button', { class: 'nav next', 'aria-label': t.next, onclick: () => this.go(1) }, icon('right'));
      this.info = h('aside', { class: 'info', hidden: !this.infoOpen });
      this.stage.append(this.bar, this.prevButton, this.nextButton, this.status, this.strip);
      this.root = h(
        'div',
        { class: 'viewer', role: 'dialog', 'aria-modal': 'true', tabindex: -1 },
        h('div', { class: 'backdrop' }, this.blur),
        this.stage,
        this.info,
      );
      document.body.append(this.root);
      // The keys go to the viewer, not to the tile it was opened from.
      this.focus();

      this.onKey = (event) => this.key(event);
      document.addEventListener('keydown', this.onKey);
      this.onResize = () => this.fit(false);
      addEventListener('resize', this.onResize);
      this.stage.addEventListener('wheel', (event) => this.wheel(event), { passive: false });
      this.stage.addEventListener('pointerdown', (event) => this.down(event));
      this.stage.addEventListener('pointermove', (event) => this.move(event));
      this.stage.addEventListener('pointerup', (event) => this.up(event));
      this.stage.addEventListener('pointercancel', (event) => this.up(event));
      this.stage.addEventListener('dblclick', (event) => this.doubleClick(event));
      this.root.addEventListener('mousemove', () => this.wake());
      this.wake();
    }

    /** Where the entry's tile is on screen, if it is drawn. */
    tileRect(entry) {
      const index = shown.findIndex((e) => e.id === entry.id);
      const row = rowOf[index];
      const item = row?.items.find((i) => i.index === index);
      if (!item) return null;
      const box = timeline.getBoundingClientRect();
      const top = box.top + row.top + item.top - timeline.scrollTop;
      if (top + item.height < box.top || top > box.bottom) return null;
      return { left: box.left + item.left, top, width: item.width, height: item.height };
    }

    show(index, { from } = {}) {
      this.index = index;
      const entry = this.entry;
      this.cleanupMedia();
      this.view = { z: 1, x: 0, y: 0 };
      this.natural = { width: entry.width || 1600, height: entry.height || 1200 };
      this.media = h('div', { class: 'media' });
      this.stage.prepend(this.media);
      this.setStatus('');
      if (entry.video) this.showVideo(entry);
      else this.showPhoto(entry);
      this.fit(false);
      if (!entry.video && faces.enabled) this.drawFaces(entry);
      if (from) this.flyFrom(from);
      this.drawBar();
      this.drawInfo();
      this.drawStrip();
      // A video has its own controls where the strip would be.
      this.strip.hidden = entry.video;
      this.prevButton.disabled = index <= 0;
      this.nextButton.disabled = index >= this.list.length - 1;
      link('thumb', entry).then((url) => {
        if (url && this.entry === entry) this.blur.style.backgroundImage = `url("${url}")`;
      });
      // The neighbours are likely next: their links (and the browser's cache) get ready.
      for (const near of [this.list[index + 1], this.list[index - 1]]) {
        if (near?.thumb) link('thumb', near);
      }
    }

    showPhoto(entry) {
      const low = h('img', { alt: '', draggable: 'false', style: 'position:absolute;inset:0;width:100%;height:100%' });
      const full = h('img', {
        alt: entry.name,
        class: 'full',
        draggable: 'false',
        style: 'position:absolute;inset:0;width:100%;height:100%',
      });
      this.media.append(low, full);
      if (entry.thumb) link('thumb', entry).then((url) => url && this.entry === entry && (low.src = url));
      const kind = entry.preview ? 'preview' : PHOTOS_SHOWN.includes(entry.mime) ? 'original' : null;
      if (!kind) return;
      this.setStatus(t.loading);
      link(kind, entry).then((url) => {
        if (!url || this.entry !== entry) return;
        full.addEventListener('load', () => {
          if (this.entry !== entry) return;
          full.classList.add('loaded');
          this.setStatus('');
          if (full.naturalWidth) this.maxZoom = Math.max(1, (full.naturalWidth / this.box.width) * 2);
        });
        full.addEventListener('error', () => this.entry === entry && this.setStatus(''));
        full.src = url;
      });
    }

    async showVideo(entry) {
      const video = h('video', {
        class: 'media-video',
        autoplay: true,
        playsinline: true,
        style: 'position:absolute;inset:0;width:100%;height:100%;background:#000',
      });
      link('thumb', entry).then((url) => url && (video.poster = url));
      this.media.append(video);
      this.video = video;
      this.player = new VideoControls(video, entry, () => {
        this.focus();
        this.wake();
      });
      this.stage.append(this.player.big, this.player.waiting, this.player.root);
      const stream = async (mode) => {
        if (mode === 'h264') this.setStatus(t.converting);
        const { url } = await api('GET', `/stream/${entry.id}?mode=${mode}`);
        if (this.entry !== entry) return;
        if (video.canPlayType('application/vnd.apple.mpegurl')) {
          video.src = url;
        } else {
          const Hls = await loadHls();
          if (this.entry !== entry || !Hls?.isSupported()) throw new Error(t.videoFailed);
          this.hls = new Hls();
          this.hls.on(Hls.Events.ERROR, (_, data) => data.fatal && this.setStatus(t.videoFailed));
          this.hls.loadSource(url);
          this.hls.attachMedia(video);
        }
        video.addEventListener('playing', () => this.setStatus(''), { once: true });
      };
      try {
        if (playsAsIs(entry)) {
          const url = await link('original', entry);
          if (this.entry !== entry) return;
          video.addEventListener(
            'error',
            () => this.entry === entry && stream('h264').catch(() => this.setStatus(t.videoFailed)),
            { once: true },
          );
          video.src = url;
        } else {
          await stream(decodes(entry) ? 'copy' : 'h264');
        }
      } catch {
        if (this.entry === entry) this.setStatus(t.videoFailed);
      }
    }

    cleanupMedia() {
      this.hls?.destroy();
      this.hls = null;
      this.player?.remove();
      this.player = null;
      if (this.video) {
        this.video.pause();
        this.video.removeAttribute('src');
        this.video.load();
        this.video = null;
      }
      this.media?.remove();
      this.maxZoom = 6;
    }

    /** The picture's box at zoom 1: as large as fits, centred. */
    fit(animate) {
      const width = this.stage.clientWidth;
      const height = this.stage.clientHeight;
      // Small pictures are not blown up beyond twice their size (videos: four times).
      const s = Math.min(width / this.natural.width, height / this.natural.height, this.entry.video ? 4 : 2);
      this.box = { width: this.natural.width * s, height: this.natural.height * s };
      this.media.style.width = `${this.box.width}px`;
      this.media.style.height = `${this.box.height}px`;
      this.view = { z: 1, x: (width - this.box.width) / 2, y: (height - this.box.height) / 2 };
      this.apply(animate);
    }

    apply(animate) {
      this.media.classList.toggle('animate', Boolean(animate));
      this.media.style.transform = `translate(${this.view.x}px, ${this.view.y}px) scale(${this.view.z})`;
      // Circles on faces keep their lines thin when zoomed in.
      this.media.style.setProperty('--z', this.view.z);
    }

    /** Circles on the photo's faces: a click opens the face's editor. */
    async drawFaces(entry) {
      const found = await facesOf(entry);
      if (this.entry !== entry) return;
      this.media.querySelector('.faces')?.remove();
      if (!found.length) return;
      const { width, height } = this.natural;
      const layer = h(
        'div',
        { class: 'faces', hidden: !facesShown },
        found.map((face) => {
          const [x, y, w, h_] = face.b;
          // A circle around the box, in parts of the picture.
          const side = Math.max(w * width, h_ * height) * 1.15;
          const left = (x + w / 2) * width - side / 2;
          const top = (y + h_ / 2) * height - side / 2;
          const person = face.p ? people.get(face.p) ?? { id: personOf(face.p), files: new Set(), cover: { file: entry.id, b: face.b } } : null;
          return h(
            'button',
            {
              class: 'face-ring' + (face.p ? '' : ' unknown'),
              style: `left:${(left / width) * 100}%;top:${(top / height) * 100}%;width:${(side / width) * 100}%;height:${(side / height) * 100}%`,
              title: face.p ? personName(face.p) : t.personTitle,
              onclick: () => {
                const shown = person && { ...person, id: personOf(person.id), files: personFiles(personOf(person.id)) };
                editPerson({ person: shown, face: { file: entry.id, i: face.i, b: face.b, p: face.p } });
              },
            },
            h('span', { class: 'face-label' }, face.p ? personName(face.p) : '?'),
          );
        }),
      );
      this.media.append(layer);
      this.drawBar();
    }

    redrawFaces() {
      const entry = this.entry;
      if (entry && !entry.video && faces.enabled) {
        this.drawFaces(entry);
        if (this.infoOpen) this.drawInfo();
      }
    }

    toggleFaces() {
      facesShown = !facesShown;
      const layer = this.media.querySelector('.faces');
      if (layer) layer.hidden = !facesShown;
      this.drawBar();
    }

    /** Opens from the tile: the picture grows out of it. */
    flyFrom(rect) {
      const stageBox = this.stage.getBoundingClientRect();
      const end = { ...this.view };
      const z = rect.width / this.box.width;
      this.view = { z, x: rect.left - stageBox.left, y: rect.top - stageBox.top + (rect.height - this.box.height * z) / 2 };
      this.apply(false);
      requestAnimationFrame(() =>
        requestAnimationFrame(() => {
          this.view = end;
          this.apply(true);
        }),
      );
    }

    /** Zooms by `factor` around the point (px, py) of the stage. */
    zoomAt(factor, px, py, animate = false) {
      if (this.entry.video) return;
      const z = Math.min(Math.max(this.view.z * factor, 1), this.maxZoom);
      const ratio = z / this.view.z;
      this.view.x = px - (px - this.view.x) * ratio;
      this.view.y = py - (py - this.view.y) * ratio;
      this.view.z = z;
      this.clampView();
      this.apply(animate);
    }

    /** Keeps the zoomed picture over the stage, centred while it is smaller. */
    clampView() {
      const width = this.stage.clientWidth;
      const height = this.stage.clientHeight;
      const w = this.box.width * this.view.z;
      const hh = this.box.height * this.view.z;
      this.view.x = w <= width ? (width - w) / 2 : Math.min(0, Math.max(width - w, this.view.x));
      this.view.y = hh <= height ? (height - hh) / 2 : Math.min(0, Math.max(height - hh, this.view.y));
    }

    point(event) {
      const box = this.stage.getBoundingClientRect();
      return { x: event.clientX - box.left, y: event.clientY - box.top };
    }

    wheel(event) {
      if (this.entry.video) return;
      event.preventDefault();
      const { x, y } = this.point(event);
      this.zoomAt(Math.exp(-event.deltaY * 0.002), x, y);
    }

    doubleClick(event) {
      if (this.entry.video || event.target.closest('button')) return;
      const { x, y } = this.point(event);
      if (this.view.z > 1) {
        this.fit(true);
      } else {
        this.zoomAt(2.5, x, y, true);
      }
    }

    down(event) {
      if (event.target.closest('button, input, video, .vcontrols') || event.button > 0) return;
      this.stage.setPointerCapture(event.pointerId);
      this.pointers.set(event.pointerId, this.point(event));
      this.gesture = { start: this.point(event), view: { ...this.view }, moved: false };
      if (this.pointers.size === 2) {
        const [a, b] = [...this.pointers.values()];
        this.gesture.pinch = { distance: Math.hypot(a.x - b.x, a.y - b.y), z: this.view.z };
      }
    }

    move(event) {
      if (!this.pointers.has(event.pointerId)) return;
      this.pointers.set(event.pointerId, this.point(event));
      const gesture = this.gesture;
      if (this.pointers.size === 2 && gesture.pinch) {
        const [a, b] = [...this.pointers.values()];
        const distance = Math.hypot(a.x - b.x, a.y - b.y);
        const target = gesture.pinch.z * (distance / gesture.pinch.distance);
        this.zoomAt(target / this.view.z, (a.x + b.x) / 2, (a.y + b.y) / 2);
        gesture.moved = true;
        return;
      }
      const p = this.point(event);
      const dx = p.x - gesture.start.x;
      const dy = p.y - gesture.start.y;
      if (Math.abs(dx) + Math.abs(dy) > 4) gesture.moved = true;
      if (this.view.z > 1) {
        this.view.x = gesture.view.x + dx;
        this.view.y = gesture.view.y + dy;
        this.clampView();
        this.apply(false);
      } else {
        // Swiping: the picture follows the finger sideways.
        this.view.x = gesture.view.x + dx;
        this.apply(false);
      }
    }

    up(event) {
      if (!this.pointers.has(event.pointerId)) return;
      this.pointers.delete(event.pointerId);
      const gesture = this.gesture;
      if (this.pointers.size || !gesture) return;
      this.gesture = null;
      if (gesture.pinch) {
        if (this.view.z <= 1.02) this.fit(true);
        return;
      }
      if (this.view.z > 1) return;
      const dx = this.point(event).x - gesture.start.x;
      if (dx > 80 && this.index > 0) this.go(-1);
      else if (dx < -80 && this.index < this.list.length - 1) this.go(1);
      else if (gesture.moved) this.fit(true);
      else if (!event.target.closest('.media')) this.close();
    }

    go(step) {
      const index = this.index + step;
      if (index < 0 || index >= this.list.length) return;
      this.show(index);
    }

    key(event) {
      // A dialog over the viewer has the keys (Escape closes only it).
      if (document.querySelector('dialog[open]')) return;
      // A slider moves with its own arrow keys.
      if (event.target.matches?.('input[type=range]') && event.key.startsWith('Arrow')) return;
      if (event.target.closest?.('input:not([type=range]), textarea')) return;
      if (this.player && this.videoKey(event)) {
        event.preventDefault();
        this.wake();
        return;
      }
      if (event.key === 'Escape') this.close();
      else if (event.key === 'ArrowLeft') this.go(-1);
      else if (event.key === 'ArrowRight') this.go(1);
      else if (event.key === 'i') this.toggleInfo();
      else if (event.key === 'f') toggleFavorite(this.entry);
      else if (event.key === 'Delete') this.deleteCurrent();
      else return;
      event.preventDefault();
    }

    /** Keys of the player; false if the key isn't one. */
    videoKey(event) {
      const player = this.player;
      const key = event.key.length === 1 ? event.key.toLowerCase() : event.key;
      if (key === ' ' || key === 'k' || key === 'л') player.toggle();
      else if (key === 'm' || key === 'ь') player.toggleMute();
      else if (key === 'ArrowUp') player.changeVolume(0.1);
      else if (key === 'ArrowDown') player.changeVolume(-0.1);
      else if (key === 'j' || key === 'о') player.seekBy(-10);
      else if (key === 'l' || key === 'д') player.seekBy(10);
      else if (key === 'ArrowLeft' && event.shiftKey) player.seekBy(-5);
      else if (key === 'ArrowRight' && event.shiftKey) player.seekBy(5);
      else return false;
      return true;
    }

    /** The bars hide after a while without the mouse moving; not over a paused video. */
    wake() {
      this.root.classList.remove('idle');
      clearTimeout(this.idleTimer);
      this.idleTimer = setTimeout(() => {
        if (!this.video || !this.video.paused) this.root.classList.add('idle');
      }, 3000);
    }

    setStatus(text) {
      this.status.textContent = text;
    }

    drawBar() {
      const entry = this.entry;
      const favorite = favorites.has(entry.id);
      this.bar.replaceChildren(
        iconButton('close', t.close, () => this.close()),
        h(
          'div',
          { class: 'title' },
          h('div', { class: 'date' }, capitalize(fullFormat.format(localDate(entry._when)))),
          h('div', { class: 'name' }, entry.name),
        ),
        iconButton('heart', favorite ? t.unfavorite : t.favorite, () => toggleFavorite(entry), {
          class: 'icon-btn' + (favorite ? ' on' : ''),
        }),
        this.media?.querySelector('.faces')
          ? iconButton('person', t.showFaces, () => this.toggleFaces(), { class: 'icon-btn' + (facesShown ? ' on' : '') })
          : null,
        iconButton('download', t.download, () => this.download()),
        iconButton('trash', t.delete, () => this.deleteCurrent()),
        iconButton('info', t.info, () => this.toggleInfo()),
      );
    }

    toggleInfo() {
      this.infoOpen = !this.infoOpen;
      this.info.hidden = !this.infoOpen;
      this.fit(false);
    }

    drawInfo() {
      const entry = this.entry;
      const row = (name, main, sub, cls) =>
        main ? h('div', { class: 'info-row' + (cls ? ` ${cls}` : '') }, icon(name), h('div', {}, h('div', { class: 'main' }, main), sub ? h('div', { class: 'sub' }, sub) : null)) : null;
      const size = [
        entry.width && entry.height ? `${entry.width} × ${entry.height}` : null,
        entry.width && entry.height && !entry.video
          ? `${((entry.width * entry.height) / 1e6).toLocaleString(intlLocale, { maximumFractionDigits: 1 })} ${t.megapixels}`
          : null,
        bytes(entry.size),
        entry.video && entry.duration ? duration(entry.duration) : null,
      ].filter(Boolean);
      const exposure = [
        entry.aperture ? `ƒ/${entry.aperture}` : null,
        entry.exposure,
        entry.focal ? `${entry.focal} ${context.locale === 'ru' ? 'мм' : 'mm'}` : null,
        entry.iso ? `ISO ${entry.iso}` : null,
      ].filter(Boolean);
      const date = localDate(entry._when);
      // replaceChildren() would write the rows that are missing as "null".
      const rows = [
        h('header', {}, h('h2', {}, t.details), iconButton('close', t.close, () => this.toggleInfo())),
        row('calendar', capitalize(fullFormat.format(date)), entry.taken ? null : t.noTakenDate),
        row('file', entry.name, [entry._format, entry.video ? entry.codec?.toUpperCase() : null, ...size].filter(Boolean).join(' · ')),
        row('folder', entry.folder),
        row('camera', entry.camera, entry.lens),
        row('aperture', exposure.join(' · ')),
        row(
          'pin',
          entry.gps ? `${entry.gps[0].toFixed(5)}, ${entry.gps[1].toFixed(5)}` : null,
          entry.gps ? t.location : null,
        ),
        row('alert', entry.error ? t.problem : null, entry.error, 'error'),
      ];
      this.info.replaceChildren(...rows.filter(Boolean));
      if (faces.enabled && !entry.video) this.drawPeople(entry);
    }

    /** Who is in the photo: a chip per person shows all their photos. */
    async drawPeople(entry) {
      const found = await facesOf(entry);
      if (this.entry !== entry || !found.length) return;
      const ids = [...new Set(found.filter((face) => face.p).map((face) => personOf(face.p)))];
      const unknown = found.length - found.filter((face) => face.p).length;
      const chips = ids.map((id) =>
        h(
          'button',
          {
            class: 'chip',
            onclick: () => {
              filters.people.clear();
              filters.people.add(id);
              this.close();
              refresh({ keepScroll: false });
            },
          },
          personName(id),
        ),
      );
      const row = h(
        'div',
        { class: 'info-row' },
        icon('person'),
        h(
          'div',
          {},
          h('div', { class: 'main people-chips' }, chips.length ? chips : t.facesFound(found.length)),
          unknown && chips.length ? h('div', { class: 'sub' }, t.facesUnknown(unknown)) : null,
        ),
      );
      // After the date and the file.
      this.info.insertBefore(row, this.info.children[3] ?? null);
    }

    drawStrip() {
      const from = Math.max(0, this.index - 7);
      const to = Math.min(this.list.length, this.index + 8);
      this.strip.replaceChildren(
        ...this.list.slice(from, to).map((entry, n) => {
          const index = from + n;
          const img = h('img', { alt: '' });
          if (entry.thumb) link('thumb', entry).then((url) => url && (img.src = url));
          return h(
            'button',
            { class: index === this.index ? 'current' : '', 'aria-label': entry.name, onclick: () => this.show(index) },
            img,
          );
        }),
      );
    }

    /** Deletes the file shown, then shows the next one (or closes). */
    async deleteCurrent() {
      const entry = this.entry;
      this.video?.pause();
      const gone = await deleteEntry(entry);
      if (viewer !== this) return;
      if (gone) {
        this.list = this.list.filter((e) => e.id !== entry.id);
        if (!this.list.length) return this.close(true);
        this.show(Math.min(this.index, this.list.length - 1));
      }
      this.focus();
    }

    async download() {
      const entry = this.entry;
      this.setStatus(t.downloading);
      try {
        await downloadEntry(entry);
        this.setStatus('');
      } catch (e) {
        this.setStatus(t.error(e.message));
      }
    }

    close(now = false) {
      document.removeEventListener('keydown', this.onKey);
      removeEventListener('resize', this.onResize);
      clearTimeout(this.idleTimer);
      if (viewer === this) viewer = null;
      // None when the last file of the list was just deleted.
      const entry = this.entry;
      const rect = now || !entry ? null : this.tileRect(entry);
      const finish = () => {
        this.cleanupMedia();
        this.root.remove();
        // Back to the tile, for the keyboard.
        if (entry) canvas.querySelector(`.tile[data-id="${entry.id}"]`)?.focus({ preventScroll: true });
      };
      if (now) return finish();
      this.root.classList.remove('open');
      this.bar.style.opacity = '0';
      if (!rect) {
        // The tile is off screen: the picture fades out.
        this.media.style.transition = 'opacity 0.2s';
        this.media.style.opacity = '0';
        setTimeout(finish, 250);
        return;
      }
      // Back into its tile.
      const stageBox = this.stage.getBoundingClientRect();
      const z = rect.width / this.box.width;
      this.view = { z, x: rect.left - stageBox.left, y: rect.top - stageBox.top + (rect.height - this.box.height * z) / 2 };
      this.apply(true);
      setTimeout(finish, 280);
    }
  }

  // ── Start ──────────────────────────────────────────────────────────

  refresh();
  await reload();
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible' && !waiting()) reload();
  });
})();
