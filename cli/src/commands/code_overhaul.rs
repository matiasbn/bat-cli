use crate::Cli;

pub fn execute(args: Cli) -> Result<&'static str, &'static str> {
    match args.option.clone().unwrap().as_ref() {
        "severity" => check_severity(args),
        "review" => check_review(args),
        "build" => check_build(args),
        _ => panic!("Wrong severity option"),
    }
}
