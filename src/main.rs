#![recursion_limit = "1024"]

use std::mem::{self, Discriminant};
use std::{str::FromStr, num::ParseIntError};
use std::ops::Add;
use std::io::Write;
use csv::{ReaderBuilder, Reader, DeserializeRecordsIntoIter};
use rustc_hash::{FxHashSet, FxHashMap};
use serde::__private::de;
use serde_json::value::Index;
use tokio::{fs::File, io::AsyncWriteExt};
use macros::struct_from_tsv;
use serde::{de::DeserializeOwned};
use typed_html::{html, text, dom::DOMTree, elements::FlowContent, elements::span, dom::TextNode, elements::td};

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
                solved = s.chars().skip(1).take(2).collect::<String>().parse::<isize>().map(|r| 99 - r );
                attempted = s.chars().skip(3).take(2).collect::<String>().parse::<isize>();
                time = s.chars().skip(5).take(5).collect::<String>().parse::<isize>();
            } 
            else { // We're dealing with the new format
                let difference = s.chars().skip(1).take(2).collect::<String>().parse::<isize>().map(|r| 99 - r);
                let missed = s.chars().skip(8).take(2).collect::<String>().parse::<isize>();
                
                solved = difference.and_then(|d| missed.clone().map(|m| d + m) );
                attempted = solved.clone().and_then(|s| missed.map(|m| s + m));
                time = s.chars().skip(3).take(5).collect::<String>().parse::<isize>();
            }

            if solved.is_ok() && attempted.is_ok() && time.is_ok() {
                output = ResultValue::Multi { time: time.unwrap(), solved: solved.unwrap(), attempted: attempted.unwrap() }
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
            ResultValue::Multi { time, solved, attempted } if solved < attempted => false,
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
    type NodeType: FlowContent<String>;
    fn to_html(&self) -> Box<Self::NodeType>;
    fn to_html_string(&self) -> String;
}

impl<T: std::fmt::Display> ToHtml for T {
    type NodeType = TextNode<String>;
    fn to_html(&self) -> Box<Self::NodeType> {
        text!("{}", self.to_string())
    }
    
    fn to_html_string(&self) -> String {
        self.to_string()
    }
}

#[derive(Clone)]
pub struct RankRow<'a, S, D, const N: usize> where S: Ord {
    pub score: S,
    pub data: [D; N],
    pub person: &'a Cuber,
}

pub struct RankPage<'a, S, D, const N: usize> where S: Ord {
    pub title: String,
    pub rows: Vec<RankRow<'a, S, D, N>>,
    pub headers: [&'static str; N]
}

impl<'a, S, D, const N: usize> RankPage<'a, S, D, N> 
where 
    S: Ord, 
    S: std::fmt::Display, 
    D: ToHtml ,
    D::NodeType: FlowContent<String> {
    pub fn to_html(&self) -> String {
        let dom: DOMTree<String> = html!(
            <html>
                <head>
                    <title> {text!("{}", self.title)} </title>
                    <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.1/dist/css/bootstrap.min.css" rel="stylesheet" crossorigin="anonymous"></link>
                    <script src="https://cdn.jsdelivr.net/npm/bootstrap@5.3.1/dist/js/bootstrap.bundle.min.js" integrity="sha384-HwwvtgBNo3bZJJLYd8oVXjrBZt8cqVSpeBNS5n7C8IVInixGAoxmnlMuBnhbgrkm" crossorigin="anonymous"></script>
                </head>
                <body data-bs -theme = "dark">
                    <h1> {text!("{}", self.title)} </h1>
                    <table class="table table-striped" >
                        <tr>
                            <th> "Rank" </th>
                            <th> "Competitor" </th>
                            <th> {text!("{}", self.title)} </th>
                            {self.headers.map(|s| html!(<th> {text!("{}", s)} </th>))}
                        </tr>
                        
                        {self.rows.iter().enumerate().map(|(i, row)| html!( 
                            <tr>  
                                <td>{i.to_html()}</td>
                                <td> {text!("{}", row.person.name)} </td>
                                <td> {text!("{}", row.score )} </td>
                                {row.data.iter().map(|d| html!(<td> {d.to_html_string()} </td>) )}
                            </tr> 
                            )
                        )}
                    </table>
                </body>
            </html>
        );
        dom.to_string()
    }

    pub fn to_html_file(&self, path: &str) {
        let mut output = std::fs::File::create(path);
        if output.is_ok() {
            output.unwrap().write_all(self.to_html().as_bytes());
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    // let info = reqwest::get("https://www.worldcubeassociation.org/api/v0/export/public")
    //     .await?
    //     .json::<serde_json::Value>()
    //     .await?;

    // let mut url = info.get("tsv_url").unwrap().as_str().unwrap();

    // let mut tmp = tempfile::tempfile().unwrap();

    // let zipped = reqwest::get(url).await?.bytes().await?;
    // tmp.write_all(&zipped[..]);
    // let mut zip = zip::ZipArchive::new(tmp).unwrap();
    // zip.extract("./data");

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

    let (sor_avg, sor_single) = get_sor_pages(&wa_cubers);
    sor_avg.to_html_file("sor_avg.html");
    sor_single.to_html_file("sor_single.html");

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
    type NodeType = span<String>;
    fn to_html(&self) -> Box<span<String>> {
        match self {
            SORRank::Blank => html!(<span> </span>),
            SORRank::Normal(r) => html!(<span> {text!("{}", r)} </span>),
            SORRank::Default(r) => html!(<span style="color: var(--bs-orange);"> {text!("{}", r)} </span>)
        }
    }

    fn to_html_string(&self) -> String {
        match self {
            SORRank::Blank => "<span> </span>".to_string(),
            SORRank::Normal(r) => format!("<span> {} </span>", r),
            SORRank::Default(r) => format!("<span style=\"color: var(--bs-orange);\"> {} </span>", r)
        }
    }
}

fn rank_for_event(rows: &mut Vec<RankRow<usize, SORRank, 18>>, event: &Event, key: fn(&Cuber, Event) -> ResultValue ) {
    rows.sort_by_key(|r| key(r.person, *event));

    for i in 0..rows.len() {
        let cur_result = key(rows[i].person, *event);
        let mut rank = 0;
        if i > 0 && cur_result == key(rows[i-1].person, *event) { // if current == previous, copy previous rank
            rank = rows[i-1].data[*event as usize].get_value();
        }
        else {
            rank = i + 1;
        }

        // if invalid result and equal last, use default enum option
        if !cur_result.valid() && cur_result == key(rows[rows.len() -1].person, *event) { 
            rows[i].data[*event as usize ] = SORRank::Default(rank);
        }
        else {
            rows[i].data[*event as usize ] = SORRank::Normal(rank);
        }
    }
}

fn get_sor_pages(cubers: &Vec<Cuber>) -> (RankPage<usize, SORRank, 18>, RankPage<usize, SORRank, 18>) {
    // Make vecs of rows (row for each cuber, vecs for sor single and average)
    let mut sor_rows_avg = cubers.iter()
        .map(|c| RankRow { score: usize::MAX, data: [SORRank::Blank; 18], person: c } )
        .collect::<Vec<RankRow<usize, SORRank, 18>>>();
    let mut sor_rows_single = sor_rows_avg.to_vec();

    // Loop through events and sort row vecs by PR single/avg in that event
    for event in Event::iter() {
        if(*event != Event::_3mbld) {
            rank_for_event(&mut sor_rows_avg, event, |c, e| c.get_average(e));
        }

        rank_for_event(&mut sor_rows_single, event, |c, e| c.get_single(e));
    }

    for i in 0..cubers.len() {
        sor_rows_avg[i].score = sor_rows_avg[i].data.iter().map(|d| d.get_value()).sum();
        sor_rows_single[i].score = sor_rows_single[i].data.iter().map(|d| d.get_value()).sum();
    }
        
    sor_rows_avg.sort_by_key(|r| r.score);
    sor_rows_single.sort_by_key(|r| r.score);

    ( RankPage { title: "SOR (Average)".to_string(), rows: sor_rows_avg, headers: Event::EVENTS }, 
    RankPage { title: "SOR (Single)".to_string(), rows: sor_rows_single, headers: Event::EVENTS } )
    
}