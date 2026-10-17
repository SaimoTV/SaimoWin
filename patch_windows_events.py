import re

with open("src/eventos.rs", "r") as f:
    text = f.read()

# Update baixar function
new_baixar = """pub fn baixar() -> Option<Vec<Evento>> {
    let json = crate::rede::json("https://embedtv.cc/api/events")?;
    let mut eventos: Vec<Evento> = serde_json::from_value(json).ok()?;
    
    let now = chrono::Utc::now();
    eventos.retain(|e| {
        let mut fim = chrono::DateTime::parse_from_rfc3339(&e.time_end).ok();
        let inicio = chrono::DateTime::parse_from_rfc3339(&e.time_start).ok();
        
        if fim.is_none() {
            if let Some(start) = inicio {
                fim = Some(start + chrono::Duration::hours(2));
            }
        }
        
        if let Some(final_fim) = fim {
            final_fim.with_timezone(&chrono::Utc) > now
        } else {
            true
        }
    });
    
    Some(eventos)
}"""

text = re.sub(
    r'pub fn baixar\(\) -> Option<Vec<Evento>> \{.*?\}',
    new_baixar.strip(),
    text,
    flags=re.DOTALL
)

# Add horario format
event_struct = """pub struct Evento {
    pub id: u32,
    pub slug: String,
    pub title: String,
    pub league: Liga,
    pub teams: Times,
    pub time_start: String,
    pub time_end: String,
    pub players: Vec<String>,
}

impl Evento {
    pub fn horario(&self) -> String {
        self.time_start.split('T').nth(1).and_then(|s| s.get(0..5)).unwrap_or("").to_string()
    }
}"""

text = re.sub(
    r'pub struct Evento \{.*?pub players: Vec<String>,\n\}',
    event_struct,
    text,
    flags=re.DOTALL
)

with open("src/eventos.rs", "w") as f:
    f.write(text)

with open("src/tela.rs", "r") as f:
    tela = f.read()

# Update UI to show time
time_ui = """                ui.painter().text(
                    egui::Pos2::new(rect.left() + 10.0, centro_y - 20.0),
                    egui::Align2::LEFT_CENTER,
                    &evento.title,
                    forte(14.0),
                    TEXTO,
                );

                ui.painter().text(
                    egui::Pos2::new(rect.left() + 10.0, centro_y),
                    egui::Align2::LEFT_CENTER,
                    &evento.horario(),
                    forte(12.0),
                    Color32::from_rgb(0, 255, 0),
                );

                ui.painter().text(
                    egui::Pos2::new(rect.left() + 10.0, centro_y + 20.0),
                    egui::Align2::LEFT_CENTER,
                    &evento.league.name,
                    normal(12.0),
                    SECUNDARIO,
                );"""

tela = re.sub(
    r'ui\.painter\(\)\.text\(\n\s*egui::Pos2::new\(rect\.left\(\) \+ 10\.0, centro_y - 12\.0\),[\s\S]*?SECUNDARIO,\n\s*\);',
    time_ui.strip(),
    tela
)

with open("src/tela.rs", "w") as f:
    f.write(tela)
