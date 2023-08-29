#![recursion_limit = "1024"]
use std::io::Write;
use csv::{ReaderBuilder, DeserializeRecordsIntoIter};
use rustc_hash::{FxHashSet, FxHashMap};
use macros::struct_from_tsv;

pub trait Table {
    const PATH: & 'static str;
    fn Read<'a>() -> Result<DeserializeRecordsIntoIter<std::fs::File, Self>, csv::Error> 
    where Self: Sized + serde::de::DeserializeOwned {
        ReaderBuilder::new().delimiter(b'\t').from_path(Self::PATH).map(|r| r.into_deserialize::<Self>())
    }
}

// This macro syntax is heinous and inflexible (but I wanted to try writing a macro and this was a simple opportunity)
// The first value is the name of the struct, and the header info from the TSV can just be pasted in 
// to create the struct fields
struct_from_tsv!(WCACompetition id	name	cityName	countryId	information	venue	venueAddress	venueDetails	external_website	cellName	latitude	longitude	cancelled	eventSpecs	wcaDelegate	organiser	year	month	day	endMonth	endDay);
impl Table for WCACompetition {
    const PATH: & 'static str = "./data/WCA_export_Competitions.tsv";
}

struct_from_tsv!(WCAPerson subid   name    countryId       gender  id);
impl Table for WCAPerson {
    const PATH: & 'static str = "./data/WCA_export_Persons.tsv";
}

struct_from_tsv!(WCAResult competitionId   eventId roundTypeId     pos     best    average personName      personId        formatId        value1 value2   value3  value4  value5  regionalSingleRecord    regionalAverageRecord   personCountryId);
impl Table for WCAResult {
    const PATH: & 'static str = "./data/WCA_export_Results.tsv";
}

impl WCAResult {
    pub fn get_average(&self) -> ResultValue {
        let mut output: ResultValue = ResultValue::None;
        if self.eventId == "333mbo" || self.eventId == "333mbf" {
            output = ResultValue::None;
        }
        else {
            output = ResultValue::from_str(&self.eventId, &self.average);
        }
        output
    }

    pub fn get_single(&self) -> ResultValue {
        ResultValue::from_str(&self.eventId, &self.best)
    }
}

#[derive(Clone, Copy)]
pub enum ResultValue {
    Time(isize),
    Moves(isize),
    Multi { time: isize, solved: isize, attempted: isize },
    DNF,
    DNS,
    None,
}

// Todo: Implement custom error type when parsing fails (instead of defaulting to none)

impl ResultValue {
    pub fn from_str(eventId: &str, s: &str) -> Self {
        let mut output: Self = ResultValue::None;
        if eventId == "333mbf" || eventId == "333mbo" { // This whole if clause is smelly
            let (mut solved, mut attempted, mut time) = ("x".parse::<isize>(), "x".parse::<isize>(), "x".parse::<isize>()); 
            if eventId == "333mbo" {
                solved = s.chars().take(2).collect::<String>().parse::<isize>().map(|r| 99 - r );
                attempted = s.chars().skip(2).take(2).collect::<String>().parse::<isize>();
                time = s.chars().skip(4).take(5).collect::<String>().parse::<isize>();
            } 
            else { // We're dealing with the new format
                let difference = s.chars().take(2).collect::<String>().parse::<isize>().map(|r| 99 - r);
                let missed = s.chars().skip(7).take(2).collect::<String>().parse::<isize>();
                
                solved = difference.and_then(|d| missed.clone().map(|m| d + m) );
                attempted = solved.clone().and_then(|s| missed.map(|m| s + m));
                time = s.chars().skip(2).take(5).collect::<String>().parse::<isize>();
            }

            if solved.is_ok() && attempted.is_ok() && time.is_ok() {
                output = ResultValue::Multi { time: time.unwrap(), solved: solved.unwrap(), attempted: attempted.unwrap() };
            }
            else {
                output = ResultValue::None;
            }
        }
        else {
            let num = s.parse::<isize>();
            output = match num {
                Ok(-1)                                 => ResultValue::DNF,
                Ok(-2)                                 => ResultValue::DNS,
                Ok(0)                                  => ResultValue::None,
                _ if eventId == "333fm" && num.is_ok() => ResultValue::Moves(num.unwrap()),
                _ if num.is_ok()                       => ResultValue::Time(num.unwrap()),
                _                                      => ResultValue::None,
            }
        }   
        output
    }
    
    // Helper for comparing results of different types
    // Negative multi points used, because sorting universally assumes smaller is better
    fn tupleify(&self) -> (isize, isize, isize, isize) { // (rank, moves, -points, time)
        match self {
            ResultValue::DNF | ResultValue::DNS | ResultValue::None => (4, 0, 0, 0),
            ResultValue::Moves(m)                                   => (3, *m, 0, 0),
            ResultValue::Multi { time, solved, attempted }          => (2, 0, attempted - (2 * solved), *time),
            ResultValue::Time(t)                                    => (1, 0, 0, *t)
        }
    }

    fn valid(&self) -> bool {
        match self {  // If there is a valid result, keep incrementing ranks (invalid results get equal last)
            ResultValue::DNF | ResultValue::DNS | ResultValue::None              => false,
            ResultValue::Multi { time, solved, attempted } if (attempted - solved) < 0 => false,
            _                                                                    => true,
        }
    }
}

impl PartialEq for ResultValue {
    fn eq(&self, other: &Self) -> bool {
        self.tupleify() == other.tupleify()
    }
}

impl PartialOrd for ResultValue {

    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let s = self.tupleify();
        let o = other.tupleify();

        Some(s.cmp(&o))
    }
}

impl Eq for ResultValue {}

impl Ord for ResultValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Default for ResultValue {
    fn default() -> Self {
        ResultValue::None
    }
}

impl std::fmt::Display for ResultValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {

        fn time_to_string(time: isize) -> String {
            let cs = time % 100;
            let s = (time / 100) % 60;
            let m = (time / 6000) % 60;
            let h = (time / 6000) / 60;
            let time_vec = vec![(h, ":"), (m, ":"), (s, "."), (cs, "")];
            time_vec.iter()
                .skip_while(|(val, sep)| (*val == 0) && (*sep != ".")) 
                .enumerate()
                .map(|(i, (val, sep))| if i > 0 { format!("{val:0>2}{sep}") }  else { format!("{val}{sep}") } )
                .collect::<Vec<_>>()
                .join("")
        } 

        match self {
            ResultValue::Multi { time, solved, attempted } => write!(f, "{}/{} {}", solved, attempted, time_to_string(time * 100).strip_suffix(".00").unwrap()),
            ResultValue::Time( time ) => write!(f, "{}", time_to_string(*time)),
            ResultValue::Moves( moves ) => write!(f, "{:.2}", (*moves as f64)/100.0),
            ResultValue::DNF => write!(f, "DNF"),
            ResultValue::None => write!(f, ""),
            ResultValue::DNS => write!(f, "DNS")
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Event {
    _skewb = 0,
    _2 = 1,
    _3 = 2,
    _3bld = 3,
    _3oh = 4,
    _3mbld = 5,
    _3fm = 6,
    _3ft = 7,
    _4 = 8,
    _4bld = 9,
    _5 = 10,
    _5bld = 11,
    _6 = 12,
    _7 = 13,
    _sq1 = 14,
    _pyram = 15,
    _minx = 16,
    _clock = 17,
}

impl Event {
    const EVENTS: [& 'static str; 18] = [
        "skewb", 
        "222", 
        "333", 
        "333bf", 
        "333oh", 
        "333mbld", 
        "333fm", 
        "333ft", 
        "444", 
        "444bf", 
        "555", 
        "555bf", 
        "666", 
        "777", 
        "sq1", 
        "pyram", 
        "minx", 
        "clock"
    ];

    const EVENT_DISPLAY_STRINGS: [& 'static str; 18] = [
        "Skewb", 
        "2x2", 
        "3x3", 
        "3BLD", 
        "OH", 
        "MBLD", 
        "FM", 
        "feet 🤢", 
        "4x4", 
        "4BLD", 
        "5x5", 
        "5BLD", 
        "6x6", 
        "7x7", 
        "Squan", 
        "Pyra", 
        "Mega", 
        "Clock"
    ];

    pub fn str_to_index(eventId: &str) -> usize {
        match eventId {
            "skewb" => 0,
            "222" => 1, 
            "333" => 2, 
            "333bf" => 3, 
            "333oh" => 4,
            "333mbf" | "333mbo" => 5, 
            "333fm" => 6, 
            "333ft" => 7, 
            "444" => 8,
            "444bf" => 9,
            "555" => 10, 
            "555bf" => 11, 
            "666" => 12, 
            "777" => 13, 
            "sq1" => 14,
            "pyram" => 15, 
            "minx" => 16, 
            "clock" => 17, 
            _ => 18
        }
    }

    pub fn iter() -> std::slice::Iter<'static, Self> {
        static Events: [Event; 18] = [ Event::_skewb, Event::_2, Event::_3, Event::_3bld, Event::_3oh, Event::_3mbld, Event::_3fm, Event::_3ft, Event::_4, Event::_4bld, Event::_5, Event::_5bld, Event::_6, Event::_7, Event::_sq1, Event::_pyram, Event::_minx, Event::_clock];
        Events.iter()
    }

    pub fn to_nice_str(&self) -> &str {
        Self::EVENT_DISPLAY_STRINGS[*self as usize]
    }
}

impl ToString for Event {
    fn to_string(&self) -> String {
        Self::EVENTS[ *self as usize ].to_string() 
    }
}

#[derive(Clone, Default)]
pub struct Cuber {
    pub results : Vec<WCAResult>,
    pub id : String,
    pub name : String,
    singles : [ResultValue; 18],
    averages: [ResultValue; 18],
}

impl Cuber {
    pub fn new(results: Vec<WCAResult>) -> Self {
        let name = results[0].personName.clone();
        let id = results[0].personId.clone();

        let mut out = Self {
            results,
            name,
            id,
            singles: [ResultValue::None; 18],
            averages: [ResultValue::None; 18],
        };

        let update = |current: &mut ResultValue, new: ResultValue| if new < *current { *current = new; };

        for result in out.results.iter() {
            let idx = Event::str_to_index(&result.eventId);

            if idx != 18 {
                update(&mut out.singles[idx], result.get_single());
                update(&mut out.averages[idx], result.get_average());
            }
        }

        out
    }

    pub fn get_average_from_str(&self, eventId: &str) -> ResultValue {
        let idx = Event::str_to_index(&eventId);
        if idx != 18 {
            self.averages[idx]
        }
        else {
            ResultValue::None
        }
    }

    pub fn get_single_from_str(&self, eventId: &str) -> ResultValue {
        let idx = Event::str_to_index(&eventId);
        if idx != 18 {
            self.singles[idx]
        }
        else {
            ResultValue::None
        }
    }

    pub fn get_single(&self, event: Event) -> ResultValue {
        self.singles[event as usize]
    }

    pub fn get_average(&self, event: Event) -> ResultValue {
        self.averages[event as usize]
    }
}

pub trait ToHtml {
    fn to_html_string(&self) -> String;
}

impl<T: std::fmt::Display> ToHtml for T {
    fn to_html_string(&self) -> String {
        self.to_string()
    }
}

pub trait Labelled {
    fn get_label(&self) -> String;
}

pub trait PageItem : Labelled + ToHtml {}

#[derive(Clone)]
pub struct RankRow<'a, S, D, const N: usize> where S: Ord {
    pub rank: usize,
    pub score: S,
    pub data: [D; N],
    pub person: &'a Cuber,
}

pub struct RankTable<'a, S, D, const N: usize> where S: Ord + std::fmt::Display, D: ToHtml {
    pub label: String,
    pub rows: Vec<RankRow<'a, S, D, N>>,
    pub headers: [&'static str; N]
}

impl<S: Ord + std::fmt::Display, D: ToHtml, const N: usize> Labelled for RankTable<'_, S, D, N> {
    fn get_label(&self) -> String {
        self.label.to_string()
    }
}

impl<S: Ord + std::fmt::Display, D: ToHtml, const N: usize> PageItem for RankTable<'_, S, D, N> {}

impl<S: Ord + std::fmt::Display, D: ToHtml, const N: usize> ToHtml for RankTable<'_, S, D, N> {
    fn to_html_string(&self) -> String {
        let title = &self.label;
        let headers = self.headers.map(|s| format!("<th> {s} </th>")).join("");
        let rowHtml = self.rows.iter().map(|row| {
            let rowDataHtml = row
                .data
                .iter()
                .map(|d| {let html = d.to_html_string(); format!("<td> {html} </td>") } )
                .collect::<Vec<String>>()
                .join("");
            let rank = row.rank;
            let name = &row.person.name;
            let score = &row.score;
            format!(r#"
                <tr>  
                    <td>{rank}</td>
                    <td> {name} </td>
                    <td> {score} </td>
                    {rowDataHtml}
                </tr> 
                "#)
        }).collect::<Vec<String>>().join("");

        format!(r#"
            <table class="table table-striped" >
                <tr>
                    <th> Rank </th>
                    <th> Competitor </th>
                    <th> Result </th>
                    {headers}
                </tr>
                {rowHtml}
            </table>
        "#)
    }

}


pub struct PageData {
    title: String,
    path: String,
}
pub struct Site {
    pages: Vec<PageData>,
    web_url: String,
}

impl Site {
    pub fn new(web_url: &str) -> Self {
        Site {
            pages: Vec::<PageData>::new(),
            web_url: web_url.to_string(),
        }
    }

    pub fn to_html_file<T>(&mut self, page: &RankPage<T>) where T:PageItem {
        let path = format!("docs/{}.html", page.name);
        let mut output = std::fs::File::create( path.to_string() );
    
        let title = &page.title;

        self.pages.push(PageData { title: title.to_string(), path: path });
    
        let tables = page.tables.iter()
        .map(|i| format!(r#"<div class="tab-pane fade" id="{}" role="tabpanel" tabindex="0">"#, i.get_label()).to_string() + &i.to_html_string() + "</div>")
        .collect::<Vec<String>>()
        .join("\n");
    
        let tabs = page.tables.iter()
        .map(|i| format!(r##"  
            <li class="nav-item" role="presentation">
                <button class="nav-link" id="home-tab" data-bs-toggle="tab" data-bs-target="#{}" type="button" role="tab" aria-controls="home-tab-pane" aria-selected="true">{}</button>
            </li>
            "##, i.get_label(), i.get_label())
        )
        .collect::<Vec<String>>()
        .join("\n");
        
        let page = format!(r#"
        <html>
            <head>
                <title> {title} </title>
                <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bootstrap-icons@1.10.5/font/bootstrap-icons.css">
                <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.1/dist/css/bootstrap.min.css" rel="stylesheet" crossorigin="anonymous"></link>
                <script src="https://cdn.jsdelivr.net/npm/bootstrap@5.3.1/dist/js/bootstrap.bundle.min.js" integrity="sha384-HwwvtgBNo3bZJJLYd8oVXjrBZt8cqVSpeBNS5n7C8IVInixGAoxmnlMuBnhbgrkm" crossorigin="anonymous"></script>
            </head>
            <body data-bs-theme="dark" class="p-5">
                <h1> {title} </h1> 
                <a href="../index.html" class="fs-2 btn btn-secondary m-4 position-fixed top-0 end-0" >
                    <i class="bi bi-house-door-fill"></i>
                </a>
                <ul class="nav nav-tabs role="tablist">
                    {tabs}
                </ul>
                <div class="tab-content">
                    {tables}
                </div>
            </body>
        </html>
        "#);
    
        if output.is_ok() {
            output.unwrap().write_all(page.as_bytes());
        }
    }

    pub fn gen_homepage(&self) {
        let mut output = std::fs::File::create( "index.html" );   

        let links = self.pages
        .iter()
        .map(|p| format!(r##"<a href="{}" class="list-group-item list-group-item-action">{}</a>"##, p.path, p.title, ))
        .collect::<Vec<_>>()
        .join("\n");

        let page = format!(r#"
        <html>
            <head>
                <title> WA Speedcubing Statistics </title>
                <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bootstrap-icons@1.10.5/font/bootstrap-icons.css">
                <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.1/dist/css/bootstrap.min.css" rel="stylesheet" crossorigin="anonymous"></link>
                <script src="https://cdn.jsdelivr.net/npm/bootstrap@5.3.1/dist/js/bootstrap.bundle.min.js" integrity="sha384-HwwvtgBNo3bZJJLYd8oVXjrBZt8cqVSpeBNS5n7C8IVInixGAoxmnlMuBnhbgrkm" crossorigin="anonymous"></script>
            </head>
            <body data-bs-theme="dark" class="p-5">
                <h1> WA Speedcubing Statistics </h1> 
                <div class="list-group list-group-flush">
                    {links}
                </div>
            </body>
        </html>
        "#);
    
        if output.is_ok() {
            output.unwrap().write_all(page.as_bytes());
        }
    }

}



pub struct RankPage<T> where T: PageItem {
    pub title: String,
    pub name: String,
    pub tables: Vec<T> 
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let info = reqwest::get("https://www.worldcubeassociation.org/api/v0/export/public")
        .await?
        .json::<serde_json::Value>()
        .await?;

    let mut url = info.get("tsv_url").unwrap().as_str().unwrap();

    let mut tmp = tempfile::tempfile().unwrap();

    let zipped = reqwest::get(url).await?.bytes().await?;
    tmp.write_all(&zipped[..]);
    let mut zip = zip::ZipArchive::new(tmp).unwrap();
    zip.extract("./data");

    println!("Getting WA Comps");
    let wa_comps = WCACompetition::Read()?
        .into_iter()
        .filter(|c| c.as_ref().is_ok_and(|v| v.cityName.contains("Western Australia") ))
        .map(|c| c.unwrap())
        .collect::<Vec::<WCACompetition>>();

    let wa_comp_id_hash = FxHashSet::from_iter(
        wa_comps
        .iter()
        .map(|c| c.id.clone())
    );

    println!("Getting WA Results");

    let all_wa_results = WCAResult::Read()?
        .into_iter()
        .filter(|r| r.as_ref().is_ok_and(|v| wa_comp_id_hash.contains(&v.competitionId) ))
        .map(|r| r.unwrap())
        .collect::<Vec::<WCAResult>>();

    println!("Sorting Results By Person");
    
    let mut wa_results_by_person = FxHashMap::<String, Vec<WCAResult>>::default();
    for result in all_wa_results.into_iter() {
        let p = wa_results_by_person.get_mut(&result.personId);
        if p.is_some() 
        {
            p.unwrap().push(result);
        }
        else {
            wa_results_by_person.insert(result.personId.clone(), vec![result]);
        }
    }

    println!("Adding Non-WA results to IDs with at least one WA result");

    for result in WCAResult::Read()?
    .into_iter()
    .filter(|r| 
        r.as_ref()
        .is_ok_and(|v| 
            wa_results_by_person.contains_key(&v.personId) && 
            !wa_comp_id_hash.contains(&v.competitionId)
        )
    )
    .map(|r| r.unwrap())
    .collect::<Vec::<WCAResult>>() {
        wa_results_by_person
            .get_mut(&result.personId)
            .unwrap()
            .push(result);
    }

    println!("Filtering IDs by proportion of WA competitions");

    let wa_cubers = wa_results_by_person
    .into_iter()
    .filter(|(id, results)| -> bool {
        let wa_count = results.iter().filter(|r| wa_comp_id_hash.contains(&r.competitionId) ).count();
        ((wa_count as f32)/(results.len() as f32)) > 0.5
    })
    .map(|(id,  results)| -> Cuber {
        Cuber::new(results)
    })
    .collect::<Vec::<Cuber>>();

    println!("Total:{}", wa_cubers.len());

    let mut site = Site::new("");

    let mut single_sor_hashmap = FxHashMap::from_iter(
        wa_cubers.iter()
        .map(|c|
            (
                c.id.to_owned(),
                RankRow {
                    rank: 0,
                    score: 0,
                    data: [SORRank::Blank; 18],
                    person: c
                }
            )
        )
    );

    let mut average_sor_hashmap = FxHashMap::from_iter(
        wa_cubers.iter()
        .map(|c|
            (
                c.id.to_owned(),
                RankRow {
                    rank: 0,
                    score: 0,
                    data: [SORRank::Blank; 18],
                    person: c
                }
            )
        )
    );

    for event in Event::iter() {
        let mut single_ranks = wa_cubers.iter()
            .map(|c| RankRow { score: c.get_single(*event), rank: 0 as usize, data: [0 as usize; 0], person: c} )
            .collect::<Vec<_>>();
        let mut average_ranks = wa_cubers.iter()
        .map(|c| RankRow { score: c.get_average(*event), rank: 0 as usize, data: [0 as usize; 0], person: c} )
        .collect::<Vec<_>>();
        rank(&mut single_ranks);
        rank(&mut average_ranks);
        
        for row in single_ranks.iter() {
            if single_ranks.last().unwrap().rank == row.rank {
                single_sor_hashmap.get_mut(&row.person.id).unwrap().data[*event as usize] = SORRank::Default(row.rank);
            }
            else {
                single_sor_hashmap.get_mut(&row.person.id).unwrap().data[*event as usize] = SORRank::Normal(row.rank);
            }
        }

        for row in average_ranks.iter() {
            if *event == Event::_3mbld {
                average_sor_hashmap.get_mut(&row.person.id).unwrap().data[*event as usize] = SORRank::Blank;
            }
            else if average_ranks.last().unwrap().rank == row.rank {
                average_sor_hashmap.get_mut(&row.person.id).unwrap().data[*event as usize] = SORRank::Default(row.rank);
            }
            else {
                average_sor_hashmap.get_mut(&row.person.id).unwrap().data[*event as usize] = SORRank::Normal(row.rank);
            }
        }

        let page = RankPage {
            name:  event.to_string(), 
            title: format!("WA {} Rankings", event.to_nice_str()), 
            tables: vec![
                RankTable { 
                    label: "Single".to_string(), 
                    rows: single_ranks.into_iter().filter(|r| r.score.valid()).collect::<Vec<_>>(),
                    headers: ["";0]
                },
                RankTable { 
                    label: "Average".to_string(), 
                    rows: average_ranks.into_iter().filter(|r| r.score.valid()).collect::<Vec<_>>(),
                    headers: ["";0]
                }
            ]
        };

        site.to_html_file(&page);
    }

    let mut single_sor = single_sor_hashmap
        .into_iter()
        .map(|(_, mut v)| {
            v.score = v.data.iter().map(|d| d.get_value()).sum();
            v
        })
        .collect::<Vec<_>>();
    rank(&mut single_sor);

    let mut average_sor = average_sor_hashmap
    .into_iter()
    .map(|(_, mut v)| {
        v.score = v.data.iter().map(|d| d.get_value()).sum();
        v
    })
    .collect::<Vec<_>>();
    rank(&mut average_sor);
    
    let sor_page = RankPage {
        name: "sor".to_string(),
        title: "WA Sum Of Ranks".to_string(),
        tables: vec![ 
            RankTable {
                label: "Single".to_string(),
                rows: single_sor,
                headers: Event::EVENT_DISPLAY_STRINGS
            },
            RankTable {
                label: "Average".to_string(),
                rows: average_sor,
                headers: Event::EVENT_DISPLAY_STRINGS
            }
        ]
    };

    site.to_html_file(&sor_page);

    site.gen_homepage();

    Ok(())
}

#[derive(Clone, Copy)]
enum SORRank {
    Blank,
    Normal(usize),
    Default(usize)
}

impl SORRank {
    pub fn get_value(&self) -> usize {
        match self {
            Self::Blank => 0,
            Self::Normal(n) => *n,
            Self::Default(n) => *n
        }
    }
}

impl ToHtml for SORRank {
    fn to_html_string(&self) -> String {
        match self {
            SORRank::Blank => "<span> </span>".to_string(),
            SORRank::Normal(r) => format!("<span> {} </span>", r),
            SORRank::Default(r) => format!("<span style=\"color: var(--bs-orange);\"> {} </span>", r)
        }
    }
}

fn rank<T, S, const N: usize>(rows: &mut Vec<RankRow<S, T, N>>) where S: Ord + Copy {
    rows.sort_by_key(|r| r.score);

    for i in 0..rows.len() {
        let cur_result = rows[i].score;
        let mut rank = 0;
        if i > 0 && cur_result == rows[i-1].score { // if current == previous, copy previous rank
            rank = rows[i-1].rank;
        }
        else {
            rank = i + 1;
        }

        rows[i].rank = rank;
    }
}