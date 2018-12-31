use chrono::prelude::*;
use failure::*;
use nom::*;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use structopt::*;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use walkdir::WalkDir;

fn main() {
    if let Err(e) = run() {
        eprintln!("{}", e);
    }
}

fn run() -> Result<(), Error> {
    let opts = Opts::from_args();

    match opts {
        Opts::New(new) => match new {
            New::Task => new_task()?,
        },
        Opts::Print(print) => match print {
            Print::Tasks => {
                for task in get_tasks()? {
                    task.print()?;
                }
            }
        },
    }

    Ok(())
}

fn new_task() -> Result<(), Error> {
    let mut rl = Editor::<()>::new();
    println!("What's the task?");
    let title = rl.readline(">> ")?;
    println!("What's the priority? (1 - 10)");
    let priority = rl.readline(">> ")?.parse::<u8>()?;
    println!("What's the due date? (yyyy-mm-dd)");
    let date_string = rl
        .readline(">> ")?
        .split('-')
        .map(|s| s.to_string())
        .collect::<Vec<String>>();
    let due_date = Utc.ymd(
        date_string[0].parse::<i32>()?,
        date_string[1].parse::<u32>()?,
        date_string[2].parse::<u32>()?,
    );
    println!("Any notes? (ctrl-d) to finish");
    let mut info = String::new();

    loop {
        match rl.readline(">> ") {
            Ok(line) => {
                info.push_str(&line);
                info.push('\n');
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
            e => {
                let _ = e?;
            }
        }
    }

    let task = Task {
        title,
        priority,
        due_date,
        info: info.trim().to_string(),
    };

    task.print()?;
    let t = task.to_file()?;
    let mut path = const_dir()?;
    let file_name = task.title.replace(" ", "_").to_lowercase() + ".cstf";
    path.push(file_name);
    let mut f = File::create(path)?;
    f.write_all(t.as_bytes())?;
    Ok(())
}

fn get_tasks() -> Result<Vec<Task>, Error> {
    let mut tasks = Vec::new();
    for f in WalkDir::new(const_dir()?)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| {
            e.file_name()
                .to_str()
                .map(|d| d.ends_with("cstf"))
                .unwrap_or(false)
        })
    {
        let entry = f?;

        if entry.metadata()?.is_dir() {
            continue;
        }

        let file = fs::read_to_string(entry.path())?;
        match parse_task(&file) {
            Ok((_, task)) => tasks.push(task),
            Err(_) => bail!("Unable to open tasks"),
        }
    }

    Ok(tasks)
}

fn const_dir() -> Result<PathBuf, Error> {
    let mut dir = dirs::home_dir().ok_or(format_err!("No Home Dir"))?;
    dir.push(".constellations");
    Ok(dir)
}

#[derive(StructOpt)]
#[structopt(name = "cst", about = "Organize the constellations of your mind")]
enum Opts {
    #[structopt(name = "new")]
    New(New),
    #[structopt(name = "print")]
    Print(Print),
}

#[derive(StructOpt)]
enum New {
    #[structopt(name = "task")]
    Task,
}

#[derive(StructOpt)]
enum Print {
    #[structopt(name = "tasks")]
    Tasks,
}

#[derive(Debug)]
struct Task {
    title: String,
    priority: u8,
    due_date: Date<Utc>,
    info: String,
}

impl Task {
    fn print(&self) -> Result<(), Error> {
        let mut stdout = StandardStream::stdout(ColorChoice::Always);
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Blue)))?;
        writeln!(&mut stdout, "{}", self.title)?;
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::White)))?;
        write!(&mut stdout, "Priority: ")?;
        match self.priority {
            0..=2 => stdout.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))?,
            3..=6 => stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?,
            7..=9 => stdout.set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))?,
            _ => stdout.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?,
        }
        writeln!(&mut stdout, "{}", self.priority)?;
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::White)))?;
        write!(&mut stdout, "Due Date: ")?;

        let date = (self.due_date - Utc::today()).num_days();
        if date <= 1 {
            stdout.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
        } else if date <= 7 {
            stdout.set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))?;
        } else if date <= 14 {
            stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
        } else {
            stdout.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))?;
        }
        writeln!(&mut stdout, "{}", self.due_date)?;
        stdout.reset()?;
        writeln!(&mut stdout)?;
        writeln!(&mut stdout, "{}", self.info)?;
        writeln!(&mut stdout)?;
        Ok(())
    }

    fn to_file(&self) -> Result<String, Error> {
        let mut output = String::from("title: \"");
        output.push_str(&self.title);
        output.push_str("\"\npriority: ");
        output.push_str(&self.priority.to_string());
        output.push_str("\ndue_date: ");
        let date = self
            .due_date
            .to_string()
            .replace("UTC", "")
            .replace("-", "/");
        output.push_str(&date);
        output.push_str("\ninfo: \"");
        output.push_str(&self.info);
        output.push('\"');

        Ok(output)
    }
}

named!(parse_task<&str, Task>,
  do_parse!(
    ws!(tag!("title: \"")) >>
    title: take_till!(|c| c == '"') >>
    ws!(tag!("\"")) >>
    ws!(tag!("priority: ")) >>
    priority: map_res!(take_while!(is_num), parse_priority) >>
    ws!(tag!("due_date: ")) >>
    year: map_res!(take_while!(is_num), year) >>
    tag!("/") >>
    month: map_res!(take_while!(is_num), month) >>
    tag!("/") >>
    day: map_res!(take_while!(is_num), day) >>
    ws!(tag!("info: \"")) >>
    info: take_till!(|c| c == '"') >>
    tag!("\"") >>
    (Task {
        title: title.to_string(),
        priority,
        due_date: Utc.ymd(year, month, day),
        info: info.to_string(),
    })
  )
);

fn parse_priority(input: &str) -> Result<u8, std::num::ParseIntError> {
    u8::from_str(input)
}

fn is_num(c: char) -> bool {
    c.is_digit(10)
}

fn year(input: &str) -> Result<i32, std::num::ParseIntError> {
    i32::from_str(input)
}

fn month(input: &str) -> Result<u32, std::num::ParseIntError> {
    u32::from_str(input)
}

fn day(input: &str) -> Result<u32, std::num::ParseIntError> {
    u32::from_str(input)
}
