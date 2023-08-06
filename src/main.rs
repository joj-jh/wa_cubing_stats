use std::mem;
use std::{str::FromStr, num::ParseIntError};
use std::ops::Add;
use csv::{ReaderBuilder, Reader, DeserializeRecordsIntoIter};
use rustc_hash::{FxHashSet, FxHashMap};
use tokio::{fs::File, io::AsyncWriteExt};
use macros::struct_from_tsv;
use serde::{de::DeserializeOwned};

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
                solved = s[1..3].parse::<isize>().map(|r| 99 - r );
                attempted = s[3..5].parse::<isize>();
                time = s[5..10].parse::<isize>();
            } 
            else { // We're dealing with the new format
                let difference = s[1..3].parse::<isize>().map(|r| 99 - r);
                let missed = s[8..10].parse::<isize>();
                
                solved = difference.and_then(|d| missed.clone().map(|m| d + m) );
                attempted = solved.clone().and_then(|s| missed.map(|m| s + m));
                time = s[3..8].parse::<isize>();
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

        // Time/moves/multi can be compared to dnf/dns/none, but time/moves/multi can't be compared with each other
        if (s.1 != o.1) && !((s.1 == 4) || (o.1 == 4)) {
            None
        }
        else {
            Some(s.cmp(&o))
        }
    }
}

const EVENTS: [&'static str; 19] = ["222", "333", "333bf", "333fm", "333ft", "333mbf", "333mbo", "333oh", "444", "444bf", "555", "555bf", "666", "777", "clock", "minx", "pyram", "skewb", "sq1"]; 

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
}

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
            averages: [ResultValue::None; 18]
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

    println!("WA Cubers:");
    for cuber in wa_cubers.iter() {
        println!("{}", cuber.name);
    }
    println!("Total:{}", wa_cubers.len());



    Ok(())
}