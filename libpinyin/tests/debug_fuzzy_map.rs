/// Debug test for fuzzy alternatives at the map level
use libpinyin::parser::FuzzyMap;

#[test]
fn debug_fuzzy_map_alternatives() {
    let fuzzy_map = FuzzyMap::new();
    
    let alts_zi = fuzzy_map.alternatives("zi");
    println!("Fuzzy alternatives for 'zi': {:?}", alts_zi);
    
    let alts_zhi = fuzzy_map.alternatives("zhi");
    println!("Fuzzy alternatives for 'zhi': {:?}", alts_zhi);
    
    let alts_si = fuzzy_map.alternatives("si");
    println!("Fuzzy alternatives for 'si': {:?}", alts_si);
}