#!/usr/bin/env python3
"""Append interface strings to every catalog.

Each catalog is a TSV: an empty context column, the English source and the translation.
New entries go at the end of the file, as the vault's did.

The table below holds the strings Quill 1.1.0 added (the Nextcloud commands and pane, the
Obsidian backlink and tag views, and the words they brought with them). The script is
idempotent: a string that is already in a catalog is left alone, so it can be run again after
the table is extended.

Every language needs every string: `cargo test -p wordcraft-ui-egui i18n` checks that the
catalogs agree, and the interface falls back to English for a string that is missing.
"""
import os

BASE = os.path.join(os.path.dirname(__file__), "..", "crates", "ui-egui", "src", "i18n")

# key -> {language: translation}
NEW = {
    "Connect to Nextcloud": {
        "bg": "Свързване с Nextcloud", "cs": "Připojit k Nextcloudu", "de": "Mit Nextcloud verbinden",
        "es": "Conectar con Nextcloud", "et": "Ühendus Nextcloudiga", "fr": "Se connecter à Nextcloud",
        "hu": "Kapcsolódás a Nextcloudhoz", "it": "Connessione a Nextcloud", "ja": "Nextcloud に接続",
        "pl": "Połącz z Nextcloud", "pt-br": "Conectar ao Nextcloud", "ru": "Подключение к Nextcloud",
        "sr": "Повезивање са Nextcloud-ом", "sr-latn": "Povezivanje sa Nextcloud-om",
        "tr": "Nextcloud'a bağlan", "uk": "Підключення до Nextcloud",
        "zh-hans": "连接到 Nextcloud", "zh-hant": "連線到 Nextcloud",
    },
    "Disconnect from Nextcloud": {
        "bg": "Прекъсване на връзката с Nextcloud", "cs": "Odpojit od Nextcloudu", "de": "Verbindung zu Nextcloud trennen",
        "es": "Desconectar de Nextcloud", "et": "Ühenduse katkestamine Nextcloudiga", "fr": "Se déconnecter de Nextcloud",
        "hu": "Kapcsolat megszüntetése a Nextclouddal", "it": "Disconnessione da Nextcloud", "ja": "Nextcloud との接続を解除",
        "pl": "Odłącz od Nextcloud", "pt-br": "Desconectar do Nextcloud", "ru": "Отключение от Nextcloud",
        "sr": "Прекидање везе са Nextcloud-ом", "sr-latn": "Prekidanje veze sa Nextcloud-om",
        "tr": "Nextcloud bağlantısını kes", "uk": "Відключення від Nextcloud",
        "zh-hans": "断开 Nextcloud 连接", "zh-hant": "中斷 Nextcloud 連線",
    },
    "Nextcloud Status": {
        "bg": "Състояние на Nextcloud", "cs": "Stav Nextcloudu", "de": "Nextcloud-Status",
        "es": "Estado de Nextcloud", "et": "Nextcloudi olek", "fr": "État de Nextcloud",
        "hu": "Nextcloud állapota", "it": "Stato di Nextcloud", "ja": "Nextcloud の状態",
        "pl": "Stan Nextcloud", "pt-br": "Status do Nextcloud", "ru": "Состояние Nextcloud",
        "sr": "Стање Nextcloud-а", "sr-latn": "Stanje Nextcloud-a",
        "tr": "Nextcloud durumu", "uk": "Стан Nextcloud",
        "zh-hans": "Nextcloud 状态", "zh-hant": "Nextcloud 狀態",
    },
    "Nextcloud Files": {
        "bg": "Файлове в Nextcloud", "cs": "Soubory v Nextcloudu", "de": "Nextcloud-Dateien",
        "es": "Archivos de Nextcloud", "et": "Nextcloudi failid", "fr": "Fichiers Nextcloud",
        "hu": "Nextcloud-fájlok", "it": "File di Nextcloud", "ja": "Nextcloud のファイル",
        "pl": "Pliki Nextcloud", "pt-br": "Arquivos do Nextcloud", "ru": "Файлы Nextcloud",
        "sr": "Фajлови Nextcloud-а", "sr-latn": "Fajlovi Nextcloud-a",
        "tr": "Nextcloud dosyaları", "uk": "Файли Nextcloud",
        "zh-hans": "Nextcloud 文件", "zh-hant": "Nextcloud 檔案",
    },
    "Open from Nextcloud": {
        "bg": "Отваряне от Nextcloud", "cs": "Otevřít z Nextcloudu", "de": "Aus Nextcloud öffnen",
        "es": "Abrir desde Nextcloud", "et": "Avamine Nextcloudist", "fr": "Ouvrir depuis Nextcloud",
        "hu": "Megnyitás a Nextcloudból", "it": "Apri da Nextcloud", "ja": "Nextcloud から開く",
        "pl": "Otwórz z Nextcloud", "pt-br": "Abrir do Nextcloud", "ru": "Открыть из Nextcloud",
        "sr": "Отварање из Nextcloud-а", "sr-latn": "Otvaranje iz Nextcloud-a",
        "tr": "Nextcloud'dan aç", "uk": "Відкрити з Nextcloud",
        "zh-hans": "从 Nextcloud 打开", "zh-hant": "從 Nextcloud 開啟",
    },
    "Save to Nextcloud": {
        "bg": "Записване в Nextcloud", "cs": "Uložit do Nextcloudu", "de": "In Nextcloud speichern",
        "es": "Guardar en Nextcloud", "et": "Salvestamine Nextcloudi", "fr": "Enregistrer dans Nextcloud",
        "hu": "Mentés a Nextcloudba", "it": "Salva in Nextcloud", "ja": "Nextcloud に保存",
        "pl": "Zapisz w Nextcloud", "pt-br": "Salvar no Nextcloud", "ru": "Сохранение в Nextcloud",
        "sr": "Чување у Nextcloud-у", "sr-latn": "Čuvanje u Nextcloud-u",
        "tr": "Nextcloud'a kaydet", "uk": "Зберегти в Nextcloud",
        "zh-hans": "保存到 Nextcloud", "zh-hant": "儲存到 Nextcloud",
    },
    "New Nextcloud Folder": {
        "bg": "Нова папка в Nextcloud", "cs": "Nová složka v Nextcloudu", "de": "Neuer Nextcloud-Ordner",
        "es": "Nueva carpeta de Nextcloud", "et": "Uus Nextcloudi kaust", "fr": "Nouveau dossier Nextcloud",
        "hu": "Új Nextcloud-mappa", "it": "Nuova cartella di Nextcloud", "ja": "Nextcloud の新しいフォルダー",
        "pl": "Nowy folder Nextcloud", "pt-br": "Nova pasta do Nextcloud", "ru": "Новая папка в Nextcloud",
        "sr": "Нова фасцикла у Nextcloud-у", "sr-latn": "Nova fascikla u Nextcloud-u",
        "tr": "Nextcloud'da yeni klasör", "uk": "Нова тека в Nextcloud",
        "zh-hans": "Nextcloud 新建文件夹", "zh-hant": "Nextcloud 新增資料夾",
    },
    "Delete on Nextcloud": {
        "bg": "Изтриване в Nextcloud", "cs": "Smazat v Nextcloudu", "de": "In Nextcloud löschen",
        "es": "Eliminar en Nextcloud", "et": "Kustutamine Nextcloudis", "fr": "Supprimer sur Nextcloud",
        "hu": "Törlés a Nextcloudon", "it": "Elimina da Nextcloud", "ja": "Nextcloud 上で削除",
        "pl": "Usuń w Nextcloud", "pt-br": "Excluir no Nextcloud", "ru": "Удаление в Nextcloud",
        "sr": "Брисање у Nextcloud-у", "sr-latn": "Brisanje u Nextcloud-u",
        "tr": "Nextcloud'da sil", "uk": "Видалити в Nextcloud",
        "zh-hans": "在 Nextcloud 中删除", "zh-hant": "在 Nextcloud 中刪除",
    },
    "Share on Nextcloud": {
        "bg": "Споделяне в Nextcloud", "cs": "Sdílet v Nextcloudu", "de": "In Nextcloud teilen",
        "es": "Compartir en Nextcloud", "et": "Jagamine Nextcloudis", "fr": "Partager sur Nextcloud",
        "hu": "Megosztás a Nextcloudon", "it": "Condividi su Nextcloud", "ja": "Nextcloud で共有",
        "pl": "Udostępnij w Nextcloud", "pt-br": "Compartilhar no Nextcloud", "ru": "Общий доступ в Nextcloud",
        "sr": "Дељење у Nextcloud-у", "sr-latn": "Deljenje u Nextcloud-u",
        "tr": "Nextcloud'da paylaş", "uk": "Поділитися в Nextcloud",
        "zh-hans": "在 Nextcloud 中共享", "zh-hant": "在 Nextcloud 中分享",
    },
    "Search Nextcloud Files": {
        "bg": "Търсене във файловете на Nextcloud", "cs": "Hledat v souborech Nextcloudu", "de": "Nextcloud-Dateien durchsuchen",
        "es": "Buscar en los archivos de Nextcloud", "et": "Nextcloudi failide otsimine", "fr": "Rechercher dans les fichiers Nextcloud",
        "hu": "Keresés a Nextcloud-fájlokban", "it": "Cerca nei file di Nextcloud", "ja": "Nextcloud のファイルを検索",
        "pl": "Szukaj w plikach Nextcloud", "pt-br": "Pesquisar nos arquivos do Nextcloud", "ru": "Поиск по файлам Nextcloud",
        "sr": "Претраживање у Nextcloud фајловима", "sr-latn": "Pretraga u Nextcloud fajlovima",
        "tr": "Nextcloud dosyalarında ara", "uk": "Пошук у файлах Nextcloud",
        "zh-hans": "搜索 Nextcloud 文件", "zh-hant": "搜尋 Nextcloud 檔案",
    },
    "Nextcloud File Versions": {
        "bg": "Версии на файловете в Nextcloud", "cs": "Verze souborů v Nextcloudu", "de": "Nextcloud-Dateiversionen",
        "es": "Versiones de los archivos de Nextcloud", "et": "Nextcloudi failide versioonid", "fr": "Versions des fichiers Nextcloud",
        "hu": "Nextcloud-fájlok verziói", "it": "Versioni dei file di Nextcloud", "ja": "Nextcloud のファイルバージョン",
        "pl": "Wersje plików Nextcloud", "pt-br": "Versões dos arquivos do Nextcloud", "ru": "Версии файлов Nextcloud",
        "sr": "Верзије фајлова у Nextcloud-у", "sr-latn": "Verzije fajlova u Nextcloud-u",
        "tr": "Nextcloud dosya sürümleri", "uk": "Версії файлів Nextcloud",
        "zh-hans": "Nextcloud 文件版本", "zh-hant": "Nextcloud 檔案版本",
    },
    "Wait for Nextcloud": {
        "bg": "Изчакване на Nextcloud", "cs": "Čekat na Nextcloud", "de": "Auf Nextcloud warten",
        "es": "Esperar a Nextcloud", "et": "Ootamine Nextcloudi", "fr": "Attendre Nextcloud",
        "hu": "Várakozás a Nextcloudra", "it": "Attesa di Nextcloud", "ja": "Nextcloud の応答を待つ",
        "pl": "Oczekiwanie na Nextcloud", "pt-br": "Aguardar o Nextcloud", "ru": "Ожидание Nextcloud",
        "sr": "Чекање на Nextcloud", "sr-latn": "Čekanje na Nextcloud",
        "tr": "Nextcloud'u bekle", "uk": "Очікування Nextcloud",
        "zh-hans": "等待 Nextcloud", "zh-hant": "等待 Nextcloud",
    },
    "Cloud": {
        "bg": "Облак", "cs": "Cloud", "de": "Cloud", "es": "Nube", "et": "Pilv", "fr": "Cloud",
        "hu": "Felhő", "it": "Cloud", "ja": "クラウド", "pl": "Chmura", "pt-br": "Nuvem", "ru": "Облако",
        "sr": "Облак", "sr-latn": "Oblaк", "tr": "Bulut", "uk": "Хмара",
        "zh-hans": "云", "zh-hant": "雲端",
    },
    "Nextcloud": {lang: "Nextcloud" for lang in [
        "bg", "cs", "de", "es", "et", "fr", "hu", "it", "ja", "pl", "pt-br", "ru", "sr", "sr-latn", "tr", "uk", "zh-hans", "zh-hant",
    ]},
    "Server": {
        "bg": "Сървър", "cs": "Server", "de": "Server", "es": "Servidor", "et": "Server", "fr": "Serveur",
        "hu": "Kiszolgáló", "it": "Server", "ja": "サーバー", "pl": "Serwer", "pt-br": "Servidor", "ru": "Сервер",
        "sr": "Сервер", "sr-latn": "Server", "tr": "Sunucu", "uk": "Сервер",
        "zh-hans": "服务器", "zh-hant": "伺服器",
    },
    "User name": {
        "bg": "Потребителско име", "cs": "Uživatelské jméno", "de": "Benutzername", "es": "Nombre de usuario",
        "et": "Kasutajanimi", "fr": "Nom d'utilisateur", "hu": "Felhasználónév", "it": "Nome utente",
        "ja": "ユーザー名", "pl": "Nazwa użytkownika", "pt-br": "Nome de usuário", "ru": "Имя пользователя",
        "sr": "Корисничко име", "sr-latn": "Korisničko ime", "tr": "Kullanıcı adı", "uk": "Ім'я користувача",
        "zh-hans": "用户名", "zh-hant": "使用者名稱",
    },
    "Connect": {
        "bg": "Свързване", "cs": "Připojit", "de": "Verbinden", "es": "Conectar", "et": "Ühenda",
        "fr": "Se connecter", "hu": "Kapcsolódás", "it": "Connetti", "ja": "接続", "pl": "Połącz",
        "pt-br": "Conectar", "ru": "Подключить", "sr": "Повезивање", "sr-latn": "Povezivanje",
        "tr": "Bağlan", "uk": "Підключити", "zh-hans": "连接", "zh-hant": "連線",
    },
    "Disconnect": {
        "bg": "Прекъсване на връзката", "cs": "Odpojit", "de": "Trennen", "es": "Desconectar",
        "et": "Ühenduse katkestamine", "fr": "Se déconnecter", "hu": "Kapcsolat megszüntetése",
        "it": "Disconnetti", "ja": "接続解除", "pl": "Odłącz", "pt-br": "Desconectar",
        "ru": "Отключить", "sr": "Прекидање везе", "sr-latn": "Prekidanje veze",
        "tr": "Bağlantıyı kes", "uk": "Відключити", "zh-hans": "断开连接", "zh-hant": "中斷連線",
    },
    "New folder": {
        "bg": "Нова папка", "cs": "Nová složka", "de": "Neuer Ordner", "es": "Nueva carpeta",
        "et": "Uus kaust", "fr": "Nouveau dossier", "hu": "Új mappa", "it": "Nuova cartella",
        "ja": "新しいフォルダー", "pl": "Nowy folder", "pt-br": "Nova pasta", "ru": "Новая папка",
        "sr": "Нова фасцикла", "sr-latn": "Nova fascikla", "tr": "Yeni klasör", "uk": "Нова тека",
        "zh-hans": "新建文件夹", "zh-hant": "新增資料夾",
    },
    "Folder": {
        "bg": "Папка", "cs": "Složka", "de": "Ordner", "es": "Carpeta", "et": "Kaust",
        "fr": "Dossier", "hu": "Mappa", "it": "Cartella", "ja": "フォルダー", "pl": "Folder",
        "pt-br": "Pasta", "ru": "Папка", "sr": "Фасцикла", "sr-latn": "Fascikla",
        "tr": "Klasör", "uk": "Тека", "zh-hans": "文件夹", "zh-hant": "資料夾",
    },
    "Files": {
        "bg": "Файлове", "cs": "Soubory", "de": "Dateien", "es": "Archivos", "et": "Failid",
        "fr": "Fichiers", "hu": "Fájlok", "it": "File", "ja": "ファイル", "pl": "Pliki",
        "pt-br": "Arquivos", "ru": "Файлы", "sr": "Фajлови", "sr-latn": "Fajlovi",
        "tr": "Dosyalar", "uk": "Файли", "zh-hans": "文件", "zh-hant": "檔案",
    },
    "Status": {
        "bg": "Състояние", "cs": "Stav", "de": "Status", "es": "Estado", "et": "Olek",
        "fr": "État", "hu": "Állapot", "it": "Stato", "ja": "状態", "pl": "Stan",
        "pt-br": "Status", "ru": "Состояние", "sr": "Стање", "sr-latn": "Stanje",
        "tr": "Durum", "uk": "Стан", "zh-hans": "状态", "zh-hant": "狀態",
    },
    "Versions": {
        "bg": "Версии", "cs": "Verze", "de": "Versionen", "es": "Versiones", "et": "Versioonid",
        "fr": "Versions", "hu": "Verziók", "it": "Versioni", "ja": "バージョン", "pl": "Wersje",
        "pt-br": "Versões", "ru": "Версии", "sr": "Верзије", "sr-latn": "Verzije",
        "tr": "Sürümler", "uk": "Версії", "zh-hans": "版本", "zh-hant": "版本",
    },
    "Save this document to the server as": {
        "bg": "Запиши този документ в сървъра като",
        "cs": "Uložit tento dokument na server jako",
        "de": "Dieses Dokument auf dem Server speichern als",
        "es": "Guardar este documento en el servidor como",
        "et": "Salvesta see dokument serverisse kui",
        "fr": "Enregistrer ce document sur le serveur sous",
        "hu": "A dokumentum mentése a kiszolgálóra mint",
        "it": "Salva questo documento sul server come",
        "ja": "このドキュメントをサーバーに保存する名前:",
        "pl": "Zapisz ten dokument na serwerze jako",
        "pt-br": "Salvar este documento no servidor como",
        "ru": "Сохранить этот документ на сервере как",
        "sr": "Чување овог документа на серверу као",
        "sr-latn": "Čuvanje ovog dokumenta na serveru kao",
        "tr": "Bu belgeyi sunucuya şu adla kaydet:",
        "uk": "Зберегти цей документ на сервері як",
        "zh-hans": "将此文档保存到服务器，名称为",
        "zh-hant": "將此文件儲存到伺服器，名稱為",
    },
    "Open a note to see the notes that link to it.": {
        "bg": "Отворете бележка, за да видите бележките, които сочат към нея.",
        "cs": "Otevřete poznámku a uvidíte poznámky, které na ni odkazují.",
        "de": "Öffnen Sie eine Notiz, um die Notizen zu sehen, die auf sie verweisen.",
        "es": "Abra una nota para ver las notas que la enlazan.",
        "et": "Avage märkme, et näha märkmed, mis sellele viitavad.",
        "fr": "Ouvrez une note pour voir les notes qui y renvoient.",
        "hu": "Nyisson meg egy jegyzetet, hogy lássa a rá hivatkozó jegyzeteket.",
        "it": "Apri una nota per vedere le note che vi rimandano.",
        "ja": "ノートを開くと、リンクしているノートが表示されます。",
        "pl": "Otwórz notatkę, aby zobaczyć notatki, które do niej linkują.",
        "pt-br": "Abra uma nota para ver as notas que fazem link para ela.",
        "ru": "Откройте заметку, чтобы увидеть заметки, которые на неё ссылаются.",
        "sr": "Отворите белешку да бисте видели белешке које на њу упуте.",
        "sr-latn": "Otvorite belešku da biste videli beleške koje na nju upućuju.",
        "tr": "Bağlantı veren notları görmek için bir not açın.",
        "uk": "Відкрийте нотатку, щоб побачити нотатки, які на неї посилаються.",
        "zh-hans": "打开一个笔记即可看到链接到它的笔记。",
        "zh-hant": "開啟一個筆記即可看到連結到它的筆記。",
    },
    "Open an Obsidian vault folder to see the tags its notes carry.": {
        "bg": "Отворете папка на хранилище Obsidian, за да видите етикетите на бележките в нея.",
        "cs": "Otevřete složku úložiště Obsidian, abyste viděli štítky jeho poznámek.",
        "de": "Öffnen Sie einen Obsidian-Speicherordner, um die Schlagwörter seiner Notizen zu sehen.",
        "es": "Abra una carpeta de almacén de Obsidian para ver las etiquetas de sus notas.",
        "et": "Ava Obsidiani hoidla kaust, et näha selle märkmede sildid.",
        "fr": "Ouvrez un dossier de coffre Obsidian pour voir les étiquettes de ses notes.",
        "hu": "Nyisson egy Obsidian-tár gyökeret, hogy lássa a jegyeinek címkéit.",
        "it": "Apri una cartella di archivio Obsidian per vedere le etichette delle sue note.",
        "ja": "Obsidian の保管フォルダーを開くと、メモのタグが表示されます。",
        "pl": "Otwórz folder magazynu Obsidian, aby zobaczyć tagi jego notatek.",
        "pt-br": "Abra uma pasta do cofre Obsidian para ver as etiquetas das notas.",
        "ru": "Откройте папку хранилища Obsidian, чтобы увидеть метки её заметок.",
        "sr": "Отворите фасциклу Obsidian складишта да бисте видели ознаке његових белешки.",
        "sr-latn": "Otvorite fasciklu Obsidian skladišta da biste videli oznake njegovih beleški.",
        "tr": "Notlarının etiketlerini görmek için bir Obsidian kasası klasörü açın.",
        "uk": "Відкрийте теку сховища Obsidian, щоб побачити мітки його нотаток.",
        "zh-hans": "打开 Obsidian 保管库文件夹即可看到笔记的标签。",
        "zh-hant": "開啟 Obsidian 保存庫資料夾即可看到筆記的標籤。",
    },
    "Index Vault Notes": {
        "bg": "Индексиране на бележките в хранилището", "cs": "Indexovat poznámky úložiště",
        "de": "Notizen des Speichers indizieren", "es": "Indexar las notas del almacén",
        "et": "Indekseri hoidla märkmed", "fr": "Indexer les notes du coffre",
        "hu": "A tár jegyzetekének indexelése", "it": "Indizza le note dell'archivio",
        "ja": "保管フォルダーのノートを索引", "pl": "Indeksuj notatki magazynu",
        "pt-br": "Indexar as notas do cofre", "ru": "Индексация заметок хранилища",
        "sr": "Индексирање белешки складишта", "sr-latn": "Indeksiranje beleški skladišta",
        "tr": "Kasa notlarını dizinle", "uk": "Індексація нотаток сховища",
        "zh-hans": "为保管库笔记建立索引", "zh-hant": "為保存庫筆記建立索引",
    },
    "Vault Backlinks": {
        "bg": "Обратни връзки в хранилището", "cs": "Opačné odkazy v úložišti",
        "de": "Rückverweise im Speicher", "es": "Retroenlaces del almacén",
        "et": "Hoidla tagasiühendused", "fr": "Rétroliens du coffre",
        "hu": "Visszautaló hivatkozások a tárban", "it": "Riferimenti inversi dell'archivio",
        "ja": "保管フォルダーのバックリンク", "pl": "Odnośniki wsteczne w magazynie",
        "pt-br": "Retroligações do cofre", "ru": "Обратные ссылки в хранилище",
        "sr": "Повратне везе у складишту", "sr-latn": "Povratne veze u skladištu",
        "tr": "Kasadaki geri bağlantılar", "uk": "Зворотні посилання у сховищі",
        "zh-hans": "保管库中的反向链接", "zh-hant": "保存庫中的反向連結",
    },
    "Vault Tags": {
        "bg": "Етикети в хранилището", "cs": "Štítky v úložišti",
        "de": "Schlagwörter im Speicher", "es": "Etiquetas del almacén",
        "et": "Hoidla sildid", "fr": "Étiquettes du coffre", "hu": "A tár címkéi",
        "it": "Etichette dell'archivio", "ja": "保管フォルダーのタグ", "pl": "Tagi w magazynie",
        "pt-br": "Etiquetas do cofre", "ru": "Метки в хранилище",
        "sr": "Ознаке у складишту", "sr-latn": "Oznake u skladištu",
        "tr": "Kasadaki etiketler", "uk": "Мітки у сховищі",
        "zh-hans": "保管库中的标签", "zh-hant": "保存庫中的標籤",
    },
    "Insert Note Link": {
        "bg": "Вмъкване на връзка към бележка", "cs": "Vložit odkaz na poznámku",
        "de": "Notizlink einfügen", "es": "Insertar enlace a nota",
        "et": "Märkme lingi sisestamine", "fr": "Insérer un lien vers une note",
        "hu": "Jegyzet-hivatkozás beszúrása", "it": "Inserisci collegamento a nota",
        "ja": "ノートリンクを挿入", "pl": "Wstaw odnośnik do notatki",
        "pt-br": "Inserir link para nota", "ru": "Вставка ссылки на заметку",
        "sr": "Уметање везе ка белешци", "sr-latn": "Umetanje veze ka belešci",
        "tr": "Not bağlantısı ekle", "uk": "Вставлення посилання на нотатку",
        "zh-hans": "插入笔记链接", "zh-hant": "插入筆記連結",
    },
    "Notes": {
        "bg": "Бележки", "cs": "Poznámky", "de": "Notizen", "es": "Notas", "et": "Märkmed",
        "fr": "Notes", "hu": "Jegyzetek", "it": "Note", "ja": "ノート", "pl": "Notatki",
        "pt-br": "Notas", "ru": "Заметки", "sr": "Белешке", "sr-latn": "Beleške",
        "tr": "Notlar", "uk": "Нотатки", "zh-hans": "笔记", "zh-hant": "筆記",
    },
    "Backlinks": {
        "bg": "Обратни връзки", "cs": "Opačné odkazy", "de": "Rückverweise", "es": "Retroenlaces",
        "et": "Tagasiühendused", "fr": "Rétroliens", "hu": "Visszautaló hivatkozások",
        "it": "Riferimenti inversi", "ja": "バックリンク", "pl": "Odnośniki wsteczne",
        "pt-br": "Retroligações", "ru": "Обратные ссылки", "sr": "Повратне везе",
        "sr-latn": "Povratne veze", "tr": "Geri bağlantılar", "uk": "Зворотні посилання",
        "zh-hans": "反向链接", "zh-hant": "反向連結",
    },
    "Tags": {
        "bg": "Етикети", "cs": "Štítky", "de": "Schlagwörter", "es": "Etiquetas", "et": "Sildid",
        "fr": "Étiquettes", "hu": "Címkék", "it": "Etichette", "ja": "タグ", "pl": "Tagi",
        "pt-br": "Etiquetas", "ru": "Метки", "sr": "Ознаке", "sr-latn": "Oznake",
        "tr": "Etiketler", "uk": "Мітки", "zh-hans": "标签", "zh-hant": "標籤",
    },
    "Don't check the server's certificate": {
        "bg": "Не проверявай сертификата на сървъра",
        "cs": "Neověřovat certifikát serveru",
        "de": "Zertifikat des Servers nicht prüfen",
        "es": "No comprobar el certificado del servidor",
        "et": "Ära kontrolli serveri sertifikaati",
        "fr": "Ne pas vérifier le certificat du serveur",
        "hu": "Ne ellenőrizd a kiszolgáló tanúsítványát",
        "it": "Non verificare il certificato del server",
        "ja": "サーバーの証明書を検証しない",
        "pl": "Nie sprawdzaj certyfikatu serwera",
        "pt-br": "Não verificar o certificado do servidor",
        "ru": "Не проверять сертификат сервера",
        "sr": "Не проверавати сертификат сервера",
        "sr-latn": "Ne proveravati sertifikat servera",
        "tr": "Sunucunun belirtisini doğrulama",
        "uk": "Не перевіряти сертифікат сервера",
        "zh-hans": "不检查服务器的证书",
        "zh-hant": "不檢查伺服器的憑證",
    },
    "An app password is safer than your account password.": {
        "bg": "Парола за приложение е по-безопасна от паролата на акаунта.",
        "cs": "Heslo pro aplikaci je bezpečnější než heslo účtu.",
        "de": "Ein App-Passwort ist sicherer als das Passwort Ihres Kontos.",
        "es": "Una contraseña de aplicación es más segura que la de la cuenta.",
        "et": "Rakenduse parool on turvalisem kui konto parool.",
        "fr": "Un mot de passe d'application est plus sûr que celui du compte.",
        "hu": "Az alkalmazásjelszó biztonságosabb a fiókjelszónál.",
        "it": "Una password per l'app è più sicura di quella dell'account.",
        "ja": "アプリパスワードはアカウントのパスワードより安全です。",
        "pl": "Hasło aplikacji jest bezpieczniejsze niż hasło konta.",
        "pt-br": "Uma senha de aplicativo é mais segura que a senha da conta.",
        "ru": "Пароль приложения безопаснее пароля учётной записи.",
        "sr": "Лозинка апликације је безбеднија од лозинке налога.",
        "sr-latn": "Lozinka aplikacije je bezbednija od lozinke naloga.",
        "tr": "Uygulama parolası, hesap parolasından daha güvenlidir.",
        "uk": "Пароль застосунку безпечніший за пароль облікового запису.",
        "zh-hans": "应用专用密码比账户密码更安全。",
        "zh-hant": "應用程式密碼比帳戶密碼更安全。",
    },
}

LANGS = [
    "bg", "cs", "de", "es", "et", "fr", "hu", "it", "ja", "pl", "pt-br", "ru", "sr", "sr-latn", "tr", "uk", "zh-hans", "zh-hant",
]

for lang in LANGS:
    path = os.path.join(BASE, f"{lang}.tsv")
    with open(path, encoding="utf-8") as f:
        text = f.read()
    lines = text.split("\n")
    existing = {line.split("\t")[1] for line in lines if line and not line.startswith("#") and len(line.split("\t")) > 1}
    added = 0
    for source, translations in NEW.items():
        if source in existing:
            continue
        translation = translations[lang]
        lines.append(f"\t{source}\t{translation}")
        added += 1
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))
    print(f"{lang}: {added} entries")
