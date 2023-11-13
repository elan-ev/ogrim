use ogrim_macros::xml;



fn main() {
    println!("{}", make_rss().unwrap());
}

fn make_rss() -> Result<String, ()> {
    // Make format dependent on CLI parameter.
    let format = if std::env::args().nth(1).is_some_and(|s| s == "--pretty") {
        ogrim::Format::Pretty { indentation: "  " }
    } else {
        ogrim::Format::Terse
    };

    let buf = xml!(
        #[format = format]
        <?xml version="1.0" encoding="UTF-8" ?>
        <rss
            version="2.0"
            xmlns:dc="http://purl.org/dc/elements/1.1/"
            xmlns:content="http://purl.org/rss/1.0/modules/content/"
            xmlns:atom="http://www.w3.org/2005/Atom"
            xmlns:media="http://search.yahoo.com/mrss/"
            xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd"
        >
            <channel>
                <title>"Foxxo Weekly"</>
                <link>"https://foxxo.tv/podcast"</>
                <description>"Your weekly talk about the cutest animal."</>
                <language>"en"</>
                <itunes:explicit>"false"</>
                <itunes:image href="https://foxxo.tv/cover.jpg" />
                <itunes:category text="Education" />
                <atom:link
                    href={"https://foxxo.tv/podcast/rss.xml"}
                    rel="self"
                    type="application/rss+xml"
                />
                // Both functions only return `Result` to show that using `?`
                // works here (just like `.await`).
                {|buf| emit_episodes(buf)?}
            </channel>
        </rss>
    );

    Ok(buf.into_string())
}

fn emit_episodes(doc: &mut ogrim::Document) -> Result<(), ()> {
    let episodes = [
        "The classic: red fox",
        "Visiting an arctic fox",
        "How big media tries to lure fox enthusiasts into bullshit news",
        "Fennec fox has big ears & a big heart <3",
    ];

    for title in episodes {
        xml!(doc,
            <item>
                <title>{title}</>
                // ... other RSS stuff
            </>
        );
    }

    Ok(())
}
