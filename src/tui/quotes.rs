/// Idle quotes - displayed when no active status (rotated every 30 seconds)
pub fn get_random_quote() -> &'static str {
    const QUOTES: &[&str] = &[
        // Programmers
        "\"Any fool can write code that a computer can understand. Good programmers write code that humans can understand.\" - Martin Fowler",
        "\"Debugging is twice as hard as writing code. If you write your code as cleverly as possible, you are by definition not smart enough to debug it.\" - Brian Kernighan",
        "\"First, solve the problem. Then, write the code.\" - John Johnson",
        "\"Code is read much more often than it is written.\" - Guido van Rossum",
        "\"Always code as if the guy who ends up maintaining your code will be a violent psychopath who knows where you live.\" - John Woods",
        "\"The best code is no code at all.\" - Jeff Atwood",
        "\"Programming is the art of telling another human what you want the computer to do.\" - Donald Knuth",
        "\"Every great developer you know got there by solving problems they were unqualified to solve until they actually did it.\" - Patrick Collison",
        "\"A language that doesn't affect the way you think about programming is not worth knowing.\" - Alan Perlis",
        "\"Simplicity is prerequisite for reliability.\" - Edsger W. Dijkstra",
        "\"Keep it simple, stupid.\" - Kelly Johnson",
        "\"Premature optimization is the root of all evil.\" - Donald Knuth",
        "\"Code reviews should be a conversation, not an inquisition.\" - Unknown",
        "\"Sometimes it pays to stay in bed on Monday rather than spending the rest of the week debugging Monday's code.\" - Dan Solomon",
        "\"Any code of your own that you haven't looked at for six or more months might as well have been written by someone else.\" - Eagleson's Law",
        
        // Monty Python
        "\"It's just a flesh wound!\" - Monty Python",
        "\"I didn't vote for you!\" - Monty Python",
        "\"What is the air speed velocity of an unladen swallow?\" - Monty Python",
        "\"It's not a matter of life or death. It's a matter of debug or death.\" - Monty Python (sort of)",
        "\"We are no longer the knights who say 'Ni!'... we are now the code that you must deploy.\" - Monty Python (adapted)",
        "\"You can debug anything if you're properly motivated.\" - Monty Python (inspired)",
        "\"She turned me into a newt! I got better.\" - Monty Python",
        "\"Stop right there! This is the sort of thing up with which I will not put!\" - Monty Python (Winston Churchill)",
        
        // Computer Science
        "\"There are only two kinds of programming languages: those people complain about and those nobody uses.\" - Bjarne Stroustrup",
        "\"A designer knows he has achieved perfection not when there is nothing left to add, but when there is nothing left to take away.\" - Antoine de Saint-Exupéry",
        "\"Most of you are familiar with the virtues of a programmer. There are three, of course: laziness, impatience, and hubris.\" - Larry Wall",
        "\"Talk is cheap. Show me the code.\" - Linus Torvalds",
        "\"Genius is one percent inspiration and ninety-nine percent perspiration.\" - Thomas Edison",
        "\"Technology is at best a necessary evil.\" - David Pogue",
        "\"The Internet treats censorship as a damage and routes around it.\" - John Gilmore",
        
        // Wisdom
        "\"The art of programming is the art of organizing complexity.\" - Edsger Dijkstra",
        "\"If you think you understand recursion, read this sentence again.\" - Unknown",
        "\"To understand what recursion is, you must first understand what recursion is.\" - Unknown",
        "\"Why do programmers prefer dark mode? Because light attracts bugs.\" - Unknown",
        "\"A SQL query walks into a bar, walks up to two tables and asks in a quiet voice: 'Can I join you?'\" - Unknown",
        "\"How many programmers does it take to change a light bulb? None, that's a hardware problem.\" - Unknown",
        "\"Why do Java developers wear glasses? Because they don't C#.\" - Unknown",
    ];
    
    use std::time::{SystemTime, UNIX_EPOCH};
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    // Change quote every 30 seconds
    QUOTES[((seconds / 30) as usize) % QUOTES.len()]
}
