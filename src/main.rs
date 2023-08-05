use csv::{ReaderBuilder, Reader, DeserializeRecordsIntoIter};
use rustc_hash::FxHashSet;
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

// This macro is heinous (but I wanted to try writing a macro and this was a simple opportunity)
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

pub struct Competitor {
    pub results: Vec<WCAResult>,
    pub 
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

    let wa_comps = WCACompetition::Read()?
        .into_iter()
        .filter(|c| c.as_ref().is_ok_and(|v| v.cityName.contains("Western Australia") ))
        .map(|c| c.unwrap())
        .collect::<Vec::<WCACompetition>>();

    let wa_compID_hash = FxHashSet::from_iter(
        wa_comps
        .iter()
        .map(|c| c.id.clone())
    );

    let all_wa_results = WCAResult::Read()?
        .into_iter()
        .filter(|r| r.as_ref().is_ok_and(|v| wa_compID_hash.contains(&v.competitionId) ))
        .map(|r| r.unwrap())
        .collect::<Vec::<WCAResult>>();

    let 



    Ok(())
}