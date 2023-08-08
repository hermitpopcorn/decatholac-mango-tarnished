# 刑事トラック＝漫GO **T̶̢͚͉͍̦͈̼̥̂̒̈̒̀́̇͗͗̆ͅÀ̵̼̩̺̩̤͙͎̟̙̯R̸̡̡̛͔̫͎͈̲͔̈́̒̇́̀́̀̿̓͊ͅÑ̵̨̲̹̎̃̈̎̉͐̂̾͐͘͝I̴̢͇͇̟̜̹͗͑̅S̷̟̿͊͂̒͋͂͆̀͑̄̓̉͘͘H̴̨̪̬̜̜̹̠̬̱̝̠̦̿́̓͋͆͗͒̽͒̓̈̆̚Ḛ̵̬̠̑͋͂̓̎͆̈̔̄̀̌̋̿͝͝D̴̡͇̖͈̯̯͇͚͓̲̭̠̟́͂̌̍̓̆̊̕**
*"...Georgia Tech University. Unfortunately for you, however, you are maidenless."*

## What is this?
decatholac MANGO (dM) is a Discord bot that fetches new manga chapter releases and then announce it to servers it's been registered to.

Currently it can parse from HTML, JSON and RSS.

## Commands
### Guild/Server
- `/set-as-feed-channel` to set the current channel as the feed channel. This requires "manage channels" permission.

### Job
- `/fetch` to trigger the bot to fetch for new chapters from the source.
- `/announce` to trigger the bot to announce new chapters to the feed channel.

Fetching and announcing happens periodically through a cronjob.
The two commands listed above can be used to trigger it manually.

## Parameters
- `--one-shot` to run the workers once and then quit without standing by as a Discord bot.

## Source configuration
It's kind of a pain to explain how it works so just look at `settings.sample.toml`
and files in `src/parsers/`.

## About the weird name

I'm bad at names. Google lists "tarnished" as a synonym to "rusty" which is the language of this rewrite, so that's what I chose.

Also, tarnished is a name in Elden Ring
