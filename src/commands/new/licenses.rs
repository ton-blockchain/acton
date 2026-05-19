pub(super) fn get_license_text(license: &str, year: &str, author: &str) -> Option<String> {
    let text = match license {
        "MIT" => MIT_LICENSE,
        "Apache-2.0" => APACHE_2_0_LICENSE,
        "GPL-3.0" => GPL_3_0_LICENSE,
        "BSD-3-Clause" => BSD_3_CLAUSE_LICENSE,
        "ISC" => ISC_LICENSE,
        "Unlicense" => UNLICENSE_LICENSE,
        _ => return None,
    };

    Some(
        text.replace("[year]", year)
            .replace("[fullname]", author)
            .replace("[yyyy]", year)
            .replace("[name of copyright owner]", author)
            .replace("[organization]", author),
    )
}

const MIT_LICENSE: &str = include_str!("templates/licenses/MIT.txt");

const APACHE_2_0_LICENSE: &str = include_str!("templates/licenses/Apache-2.0.txt");

const GPL_3_0_LICENSE: &str = include_str!("templates/licenses/GPL-3.0.txt");

const BSD_3_CLAUSE_LICENSE: &str = include_str!("templates/licenses/BSD-3-Clause.txt");

const ISC_LICENSE: &str = include_str!("templates/licenses/ISC.txt");

const UNLICENSE_LICENSE: &str = include_str!("templates/licenses/Unlicense.txt");
