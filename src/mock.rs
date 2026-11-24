//! Made-up library contents for the layout mockup. Nothing here touches real files.

pub struct Library {
    pub name: String,
    pub sources: Vec<String>,
    pub excluded: Vec<String>,
    pub scanned: &'static str,
}

pub struct Photo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub format: &'static str,
    /// Days since 01.01.2023, for sorting and showing the date.
    pub day: u32,
    pub library: usize,
    pub folder: String,
    pub category: usize,
    pub tags: Vec<usize>,
    pub albums: Vec<usize>,
    pub favorite: bool,
    pub description: String,
    /// Index of a test picture, if there are any.
    pub picture: Option<usize>,
}

pub struct Album {
    pub name: String,
    pub group: Option<usize>,
    pub day: u32,
    pub description: String,
    pub favorite: bool,
}

/// One file of a duplicate group: a copy of `photo` that lives somewhere else.
pub struct Copy {
    pub photo: usize,
    pub name: String,
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub format: &'static str,
}

pub struct DuplicateGroup {
    pub copies: Vec<Copy>,
    pub keep: usize,
    /// Similar pictures only: how alike they are, percent.
    pub similarity: u32,
}

pub struct Data {
    pub libraries: Vec<Library>,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub album_groups: Vec<String>,
    pub albums: Vec<Album>,
    pub photos: Vec<Photo>,
    pub exact: Vec<DuplicateGroup>,
    pub similar: Vec<DuplicateGroup>,
    pub missing: Vec<String>,
}

const PHOTO_COUNT: usize = 140;
const RATIOS: [(u32, u32); 9] = [(3, 2), (2, 3), (4, 3), (3, 4), (16, 9), (9, 16), (1, 1), (21, 9), (4, 5)];
const FORMATS: [(&str, &str); 4] = [("jpg", "JPEG"), ("png", "PNG"), ("webp", "WEBP"), ("gif", "GIF")];

/// Small deterministic generator, so the mockup looks the same on every start.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

/// `pictures` — how many test pictures there are; the photos take turns using them.
/// Their real sizes replace the made-up ones once the pictures load.
pub fn generate(pictures: usize) -> Data {
    let libraries = vec![
        Library {
            name: "Personal".into(),
            sources: strings(&[r"D:\Фото\Телефон", r"D:\Фото\Камера", r"E:\Архив\2019"]),
            excluded: strings(&[r"D:\Фото\Камера\.thumbnails"]),
            scanned: "проверено сегодня",
        },
        Library {
            name: "Anime".into(),
            sources: strings(&[r"D:\Картинки\Anime", r"D:\Загрузки\anime"]),
            excluded: Vec::new(),
            scanned: "проверено вчера",
        },
        Library {
            name: "Art".into(),
            sources: strings(&[r"D:\Арт\Референсы", r"D:\Арт\Скетчи"]),
            excluded: strings(&[r"D:\Арт\Скетчи\temp"]),
            scanned: "проверено 3 дня назад",
        },
    ];
    let categories = strings(&["Photography", "Anime", "Artwork", "Wallpapers", "Memes"]);
    let tags = strings(&[
        "Night", "Blue", "Forest", "Character", "Winter", "Landscape", "City", "Portrait", "Sunset", "Sea", "Cat",
        "Sketch",
    ]);
    let album_groups = strings(&["Путешествия", "Творчество", "Другие альбомы"]);
    let albums = vec![
        album("Япония 2025", Some(0), 820, "Токио и Киото, весна", true),
        album("Карелия", Some(0), 610, "", false),
        album("Море", Some(0), 400, "Лето у моря", false),
        album("Референсы поз", Some(1), 700, "Для набросков", true),
        album("Палитры", Some(1), 650, "", false),
        album("Обои на рабочий стол", Some(2), 900, "", true),
        album("Лучшее", Some(2), 300, "Самые удачные кадры", false),
    ];

    let mut random = Random(0x9E37_79B9_7F4A_7C15);
    let mut photos = Vec::with_capacity(PHOTO_COUNT);
    for index in 0..PHOTO_COUNT {
        let library = random.below(libraries.len());
        let picture = if pictures == 0 { None } else { Some(index % pictures) };
        let (a, b) = RATIOS[random.below(RATIOS.len())];
        let long = [1920, 2400, 3000, 4032][random.below(4)];
        let (width, height) = if a >= b { (long, long * b / a) } else { (long * a / b, long) };
        let (extension, format) = match library {
            0 => FORMATS[if random.chance(85) { 0 } else { 1 }],
            1 => FORMATS[[0, 1, 2, 3][random.below(4)]],
            _ => FORMATS[[1, 2, 0][random.below(3)]],
        };
        let name = match library {
            0 => format!("IMG_{}.{extension}", 2000 + index * 7),
            1 => format!("{}_{:03}.{extension}", ["frieren", "ghibli", "wallpaper", "screenshot"][random.below(4)], index),
            _ => format!("{}_{:02}.{extension}", ["ref_pose", "palette", "sketch", "concept"][random.below(4)], index),
        };
        let sources = &libraries[library].sources;
        let folder = sources[random.below(sources.len())].clone();
        let bytes = (width as u64 * height as u64) * [3, 4, 6, 8][random.below(4)] / 10;
        let mut tag_list = Vec::new();
        for _ in 0..1 + random.below(3) {
            let tag = random.below(tags.len());
            if !tag_list.contains(&tag) {
                tag_list.push(tag);
            }
        }
        let mut album_list = Vec::new();
        if random.chance(40) {
            album_list.push(random.below(albums.len()));
        }
        let category = match library {
            0 => [0, 0, 3, 4][random.below(4)],
            1 => [1, 1, 3][random.below(3)],
            _ => [2, 2, 3][random.below(3)],
        };
        photos.push(Photo {
            name,
            width,
            height,
            bytes,
            format,
            day: random.below(1420) as u32,
            library,
            folder,
            category,
            tags: tag_list,
            albums: album_list,
            favorite: random.chance(15),
            description: if random.chance(20) { "Снято на прогулке".into() } else { String::new() },
            picture,
        });
    }

    let exact = (0..4)
        .map(|group| {
            let photo = group * 17 + 3;
            let original = &photos[photo];
            let mut copies = vec![copy_of(photo, original, &original.folder, &original.name)];
            copies.push(copy_of(photo, original, r"D:\Загрузки", &original.name));
            if group % 2 == 0 {
                copies.push(copy_of(photo, original, r"E:\Архив\Копии", &format!("{} (1)", original.name)));
            }
            DuplicateGroup { copies, keep: 0, similarity: 100 }
        })
        .collect();

    let similar = (0..3)
        .map(|group| {
            let photo = group * 23 + 5;
            let original = &photos[photo];
            let first = copy_of(photo, original, &original.folder, &original.name);
            let stem = original.name.rsplit_once('.').map_or(original.name.as_str(), |(stem, _)| stem);
            let mut smaller = copy_of(photo, original, r"D:\Загрузки", &format!("{stem}_small.webp"));
            smaller.width = original.width / 2;
            smaller.height = original.height / 2;
            smaller.bytes = original.bytes / 5;
            smaller.format = "WEBP";
            DuplicateGroup { copies: vec![first, smaller], keep: 0, similarity: [97, 93, 88][group] }
        })
        .collect();

    let missing = strings(&[r"E:\Архив\2019\IMG_0412.jpg", r"D:\Загрузки\anime\wallpaper_031.png"]);

    Data { libraries, categories, tags, album_groups, albums, photos, exact, similar, missing }
}

fn album(name: &str, group: Option<usize>, day: u32, description: &str, favorite: bool) -> Album {
    Album { name: name.into(), group, day, description: description.into(), favorite }
}

fn copy_of(photo: usize, original: &Photo, folder: &str, name: &str) -> Copy {
    Copy {
        photo,
        name: name.into(),
        path: format!(r"{folder}\{name}"),
        width: original.width,
        height: original.height,
        bytes: original.bytes,
        format: original.format,
    }
}

/// "14.03.2025"
pub fn date_text(day: u32) -> String {
    const MONTHS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut year = 2023;
    let mut day = day;
    loop {
        let length = if year % 4 == 0 { 366 } else { 365 };
        if day < length {
            break;
        }
        day -= length;
        year += 1;
    }
    let mut month = 0;
    while month < 11 {
        let length = MONTHS[month] + u32::from(month == 1 && year % 4 == 0);
        if day < length {
            break;
        }
        day -= length;
        month += 1;
    }
    format!("{:02}.{:02}.{year}", day + 1, month + 1)
}

/// "2,4 МБ"
pub fn size_text(bytes: u64) -> String {
    let (value, unit) = if bytes >= 1 << 30 {
        (bytes as f64 / (1u64 << 30) as f64, "ГБ")
    } else if bytes >= 1 << 20 {
        (bytes as f64 / (1u64 << 20) as f64, "МБ")
    } else {
        (bytes as f64 / 1024.0, "КБ")
    };
    format!("{value:.1} {unit}").replace('.', ",")
}

/// Russian plural: `plural(5, "изображение", "изображения", "изображений")` → "5 изображений".
pub fn plural(count: usize, one: &str, few: &str, many: &str) -> String {
    let word = match (count % 10, count % 100) {
        (1, 11) => many,
        (1, _) => one,
        (2..=4, 12..=14) => many,
        (2..=4, _) => few,
        _ => many,
    };
    format!("{count} {word}")
}
