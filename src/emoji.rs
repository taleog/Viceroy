use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Emoji {
    pub emoji: String,
    pub name: String,
    pub keywords: Vec<String>,
}

// Lightweight emoji database - most commonly used emojis
pub fn get_emoji_database() -> Vec<Emoji> {
    vec![
        Emoji {
            emoji: "😀".into(),
            name: "grinning".into(),
            keywords: vec!["smile".into(), "happy".into()],
        },
        Emoji {
            emoji: "😃".into(),
            name: "smiley".into(),
            keywords: vec!["smile".into(), "happy".into()],
        },
        Emoji {
            emoji: "😄".into(),
            name: "smile".into(),
            keywords: vec!["happy".into(), "joy".into()],
        },
        Emoji {
            emoji: "😁".into(),
            name: "grin".into(),
            keywords: vec!["smile".into(), "happy".into()],
        },
        Emoji {
            emoji: "😅".into(),
            name: "sweat_smile".into(),
            keywords: vec!["hot".into(), "happy".into()],
        },
        Emoji {
            emoji: "😂".into(),
            name: "joy".into(),
            keywords: vec!["laugh".into(), "happy".into()],
        },
        Emoji {
            emoji: "🤣".into(),
            name: "rofl".into(),
            keywords: vec!["laugh".into(), "lol".into()],
        },
        Emoji {
            emoji: "😊".into(),
            name: "blush".into(),
            keywords: vec!["smile".into(), "happy".into()],
        },
        Emoji {
            emoji: "😇".into(),
            name: "innocent".into(),
            keywords: vec!["angel".into()],
        },
        Emoji {
            emoji: "🙂".into(),
            name: "slightly_smiling_face".into(),
            keywords: vec!["smile".into()],
        },
        Emoji {
            emoji: "😉".into(),
            name: "wink".into(),
            keywords: vec!["flirt".into()],
        },
        Emoji {
            emoji: "😍".into(),
            name: "heart_eyes".into(),
            keywords: vec!["love".into(), "crush".into()],
        },
        Emoji {
            emoji: "🥰".into(),
            name: "smiling_face_with_hearts".into(),
            keywords: vec!["love".into(), "hearts".into()],
        },
        Emoji {
            emoji: "😘".into(),
            name: "kissing_heart".into(),
            keywords: vec!["love".into(), "kiss".into()],
        },
        Emoji {
            emoji: "😗".into(),
            name: "kissing".into(),
            keywords: vec!["kiss".into()],
        },
        Emoji {
            emoji: "😙".into(),
            name: "kissing_smiling_eyes".into(),
            keywords: vec!["kiss".into()],
        },
        Emoji {
            emoji: "😚".into(),
            name: "kissing_closed_eyes".into(),
            keywords: vec!["kiss".into()],
        },
        Emoji {
            emoji: "🙃".into(),
            name: "upside_down_face".into(),
            keywords: vec!["sarcasm".into()],
        },
        Emoji {
            emoji: "😋".into(),
            name: "yum".into(),
            keywords: vec!["tongue".into(), "food".into()],
        },
        Emoji {
            emoji: "😛".into(),
            name: "stuck_out_tongue".into(),
            keywords: vec!["tongue".into()],
        },
        Emoji {
            emoji: "😜".into(),
            name: "stuck_out_tongue_winking_eye".into(),
            keywords: vec!["tongue".into(), "wink".into()],
        },
        Emoji {
            emoji: "🤪".into(),
            name: "zany_face".into(),
            keywords: vec!["crazy".into(), "wild".into()],
        },
        Emoji {
            emoji: "😝".into(),
            name: "stuck_out_tongue_closed_eyes".into(),
            keywords: vec!["tongue".into()],
        },
        Emoji {
            emoji: "🤑".into(),
            name: "money_mouth_face".into(),
            keywords: vec!["money".into(), "rich".into()],
        },
        Emoji {
            emoji: "🤗".into(),
            name: "hugs".into(),
            keywords: vec!["hug".into()],
        },
        Emoji {
            emoji: "🤭".into(),
            name: "hand_over_mouth".into(),
            keywords: vec!["oops".into(), "surprise".into()],
        },
        Emoji {
            emoji: "🤫".into(),
            name: "shushing_face".into(),
            keywords: vec!["quiet".into(), "shh".into()],
        },
        Emoji {
            emoji: "🤔".into(),
            name: "thinking".into(),
            keywords: vec!["think".into(), "hmm".into()],
        },
        Emoji {
            emoji: "🤐".into(),
            name: "zipper_mouth_face".into(),
            keywords: vec!["silence".into(), "secret".into()],
        },
        Emoji {
            emoji: "🤨".into(),
            name: "raised_eyebrow".into(),
            keywords: vec!["skeptical".into(), "suspicious".into()],
        },
        Emoji {
            emoji: "😐".into(),
            name: "neutral_face".into(),
            keywords: vec!["meh".into()],
        },
        Emoji {
            emoji: "😑".into(),
            name: "expressionless".into(),
            keywords: vec!["blank".into()],
        },
        Emoji {
            emoji: "😶".into(),
            name: "no_mouth".into(),
            keywords: vec!["silent".into()],
        },
        Emoji {
            emoji: "😏".into(),
            name: "smirk".into(),
            keywords: vec!["smug".into()],
        },
        Emoji {
            emoji: "😒".into(),
            name: "unamused".into(),
            keywords: vec!["unhappy".into(), "meh".into()],
        },
        Emoji {
            emoji: "🙄".into(),
            name: "roll_eyes".into(),
            keywords: vec!["whatever".into()],
        },
        Emoji {
            emoji: "😬".into(),
            name: "grimacing".into(),
            keywords: vec!["awkward".into()],
        },
        Emoji {
            emoji: "🤥".into(),
            name: "lying_face".into(),
            keywords: vec!["lie".into(), "pinocchio".into()],
        },
        Emoji {
            emoji: "😌".into(),
            name: "relieved".into(),
            keywords: vec!["whew".into(), "relief".into()],
        },
        Emoji {
            emoji: "😔".into(),
            name: "pensive".into(),
            keywords: vec!["sad".into(), "down".into()],
        },
        Emoji {
            emoji: "😪".into(),
            name: "sleepy".into(),
            keywords: vec!["tired".into(), "sleep".into()],
        },
        Emoji {
            emoji: "🤤".into(),
            name: "drooling_face".into(),
            keywords: vec!["drool".into()],
        },
        Emoji {
            emoji: "😴".into(),
            name: "sleeping".into(),
            keywords: vec!["sleep".into(), "zzz".into()],
        },
        Emoji {
            emoji: "😷".into(),
            name: "mask".into(),
            keywords: vec!["sick".into(), "doctor".into()],
        },
        Emoji {
            emoji: "🤒".into(),
            name: "face_with_thermometer".into(),
            keywords: vec!["sick".into(), "ill".into()],
        },
        Emoji {
            emoji: "🤕".into(),
            name: "face_with_head_bandage".into(),
            keywords: vec!["hurt".into(), "injured".into()],
        },
        Emoji {
            emoji: "🤢".into(),
            name: "nauseated_face".into(),
            keywords: vec!["sick".into(), "gross".into()],
        },
        Emoji {
            emoji: "🤮".into(),
            name: "vomiting_face".into(),
            keywords: vec!["sick".into(), "puke".into()],
        },
        Emoji {
            emoji: "🤧".into(),
            name: "sneezing_face".into(),
            keywords: vec!["achoo".into(), "sick".into()],
        },
        Emoji {
            emoji: "🥵".into(),
            name: "hot_face".into(),
            keywords: vec!["hot".into(), "heat".into()],
        },
        Emoji {
            emoji: "🥶".into(),
            name: "cold_face".into(),
            keywords: vec!["cold".into(), "freezing".into()],
        },
        Emoji {
            emoji: "😎".into(),
            name: "sunglasses".into(),
            keywords: vec!["cool".into()],
        },
        Emoji {
            emoji: "🤓".into(),
            name: "nerd_face".into(),
            keywords: vec!["geek".into(), "nerd".into()],
        },
        Emoji {
            emoji: "🧐".into(),
            name: "monocle_face".into(),
            keywords: vec!["curious".into()],
        },
        Emoji {
            emoji: "😕".into(),
            name: "confused".into(),
            keywords: vec!["huh".into(), "what".into()],
        },
        Emoji {
            emoji: "😟".into(),
            name: "worried".into(),
            keywords: vec!["concerned".into(), "nervous".into()],
        },
        Emoji {
            emoji: "🙁".into(),
            name: "slightly_frowning_face".into(),
            keywords: vec!["sad".into()],
        },
        Emoji {
            emoji: "☹️".into(),
            name: "frowning_face".into(),
            keywords: vec!["sad".into()],
        },
        Emoji {
            emoji: "😮".into(),
            name: "open_mouth".into(),
            keywords: vec!["wow".into(), "surprised".into()],
        },
        Emoji {
            emoji: "😯".into(),
            name: "hushed".into(),
            keywords: vec!["wow".into(), "surprised".into()],
        },
        Emoji {
            emoji: "😲".into(),
            name: "astonished".into(),
            keywords: vec!["shocked".into(), "wow".into()],
        },
        Emoji {
            emoji: "😳".into(),
            name: "flushed".into(),
            keywords: vec!["embarrassed".into()],
        },
        Emoji {
            emoji: "🥺".into(),
            name: "pleading_face".into(),
            keywords: vec!["puppy".into(), "please".into()],
        },
        Emoji {
            emoji: "😦".into(),
            name: "frowning".into(),
            keywords: vec!["sad".into()],
        },
        Emoji {
            emoji: "😧".into(),
            name: "anguished".into(),
            keywords: vec!["stunned".into()],
        },
        Emoji {
            emoji: "😨".into(),
            name: "fearful".into(),
            keywords: vec!["scared".into(), "fear".into()],
        },
        Emoji {
            emoji: "😰".into(),
            name: "cold_sweat".into(),
            keywords: vec!["nervous".into()],
        },
        Emoji {
            emoji: "😥".into(),
            name: "disappointed_relieved".into(),
            keywords: vec!["phew".into(), "sad".into()],
        },
        Emoji {
            emoji: "😢".into(),
            name: "cry".into(),
            keywords: vec!["sad".into(), "tear".into()],
        },
        Emoji {
            emoji: "😭".into(),
            name: "sob".into(),
            keywords: vec!["cry".into(), "sad".into()],
        },
        Emoji {
            emoji: "😱".into(),
            name: "scream".into(),
            keywords: vec!["munch".into(), "scared".into()],
        },
        Emoji {
            emoji: "😖".into(),
            name: "confounded".into(),
            keywords: vec!["confused".into()],
        },
        Emoji {
            emoji: "😣".into(),
            name: "persevere".into(),
            keywords: vec!["struggle".into()],
        },
        Emoji {
            emoji: "😞".into(),
            name: "disappointed".into(),
            keywords: vec!["sad".into()],
        },
        Emoji {
            emoji: "😓".into(),
            name: "sweat".into(),
            keywords: vec!["hot".into()],
        },
        Emoji {
            emoji: "😩".into(),
            name: "weary".into(),
            keywords: vec!["tired".into()],
        },
        Emoji {
            emoji: "😫".into(),
            name: "tired_face".into(),
            keywords: vec!["tired".into(), "exhausted".into()],
        },
        Emoji {
            emoji: "🥱".into(),
            name: "yawning_face".into(),
            keywords: vec!["tired".into(), "bored".into()],
        },
        Emoji {
            emoji: "😤".into(),
            name: "triumph".into(),
            keywords: vec!["smug".into(), "proud".into()],
        },
        Emoji {
            emoji: "😡".into(),
            name: "rage".into(),
            keywords: vec!["angry".into(), "mad".into()],
        },
        Emoji {
            emoji: "😠".into(),
            name: "angry".into(),
            keywords: vec!["mad".into()],
        },
        Emoji {
            emoji: "🤬".into(),
            name: "cursing".into(),
            keywords: vec!["swear".into(), "censor".into()],
        },
        Emoji {
            emoji: "👍".into(),
            name: "thumbsup".into(),
            keywords: vec!["yes".into(), "ok".into(), "agree".into()],
        },
        Emoji {
            emoji: "👎".into(),
            name: "thumbsdown".into(),
            keywords: vec!["no".into(), "disagree".into()],
        },
        Emoji {
            emoji: "👌".into(),
            name: "ok_hand".into(),
            keywords: vec!["ok".into(), "perfect".into()],
        },
        Emoji {
            emoji: "✌️".into(),
            name: "v".into(),
            keywords: vec!["peace".into(), "victory".into()],
        },
        Emoji {
            emoji: "🤞".into(),
            name: "crossed_fingers".into(),
            keywords: vec!["luck".into(), "hope".into()],
        },
        Emoji {
            emoji: "🤟".into(),
            name: "love_you_gesture".into(),
            keywords: vec!["love".into()],
        },
        Emoji {
            emoji: "🤘".into(),
            name: "metal".into(),
            keywords: vec!["rock".into()],
        },
        Emoji {
            emoji: "👋".into(),
            name: "wave".into(),
            keywords: vec!["hi".into(), "hello".into(), "bye".into()],
        },
        Emoji {
            emoji: "🤚".into(),
            name: "raised_back_of_hand".into(),
            keywords: vec!["stop".into()],
        },
        Emoji {
            emoji: "🖐️".into(),
            name: "raised_hand_with_fingers_splayed".into(),
            keywords: vec!["stop".into(), "high_five".into()],
        },
        Emoji {
            emoji: "✋".into(),
            name: "hand".into(),
            keywords: vec!["stop".into(), "high_five".into()],
        },
        Emoji {
            emoji: "🖖".into(),
            name: "vulcan_salute".into(),
            keywords: vec!["spock".into()],
        },
        Emoji {
            emoji: "👏".into(),
            name: "clap".into(),
            keywords: vec!["applause".into(), "congrats".into()],
        },
        Emoji {
            emoji: "🙌".into(),
            name: "raised_hands".into(),
            keywords: vec!["hooray".into(), "yay".into()],
        },
        Emoji {
            emoji: "👐".into(),
            name: "open_hands".into(),
            keywords: vec!["hug".into()],
        },
        Emoji {
            emoji: "🤲".into(),
            name: "palms_up_together".into(),
            keywords: vec!["pray".into()],
        },
        Emoji {
            emoji: "🤝".into(),
            name: "handshake".into(),
            keywords: vec!["deal".into(), "agreement".into()],
        },
        Emoji {
            emoji: "🙏".into(),
            name: "pray".into(),
            keywords: vec!["please".into(), "thanks".into(), "namaste".into()],
        },
        Emoji {
            emoji: "✍️".into(),
            name: "writing_hand".into(),
            keywords: vec!["write".into()],
        },
        Emoji {
            emoji: "💪".into(),
            name: "muscle".into(),
            keywords: vec!["strong".into(), "flex".into()],
        },
        Emoji {
            emoji: "🦾".into(),
            name: "mechanical_arm".into(),
            keywords: vec!["robot".into()],
        },
        Emoji {
            emoji: "🦿".into(),
            name: "mechanical_leg".into(),
            keywords: vec!["robot".into()],
        },
        Emoji {
            emoji: "🦵".into(),
            name: "leg".into(),
            keywords: vec![],
        },
        Emoji {
            emoji: "🦶".into(),
            name: "foot".into(),
            keywords: vec![],
        },
        Emoji {
            emoji: "👂".into(),
            name: "ear".into(),
            keywords: vec!["hear".into()],
        },
        Emoji {
            emoji: "🦻".into(),
            name: "ear_with_hearing_aid".into(),
            keywords: vec![],
        },
        Emoji {
            emoji: "👃".into(),
            name: "nose".into(),
            keywords: vec!["smell".into()],
        },
        Emoji {
            emoji: "🧠".into(),
            name: "brain".into(),
            keywords: vec!["smart".into()],
        },
        Emoji {
            emoji: "🦷".into(),
            name: "tooth".into(),
            keywords: vec!["dentist".into()],
        },
        Emoji {
            emoji: "🦴".into(),
            name: "bone".into(),
            keywords: vec![],
        },
        Emoji {
            emoji: "👀".into(),
            name: "eyes".into(),
            keywords: vec!["look".into(), "see".into()],
        },
        Emoji {
            emoji: "👁️".into(),
            name: "eye".into(),
            keywords: vec!["look".into()],
        },
        Emoji {
            emoji: "👅".into(),
            name: "tongue".into(),
            keywords: vec![],
        },
        Emoji {
            emoji: "👄".into(),
            name: "lips".into(),
            keywords: vec!["kiss".into()],
        },
        Emoji {
            emoji: "💋".into(),
            name: "kiss".into(),
            keywords: vec!["lipstick".into()],
        },
        Emoji {
            emoji: "❤️".into(),
            name: "heart".into(),
            keywords: vec!["love".into()],
        },
        Emoji {
            emoji: "🧡".into(),
            name: "orange_heart".into(),
            keywords: vec!["love".into()],
        },
        Emoji {
            emoji: "💛".into(),
            name: "yellow_heart".into(),
            keywords: vec!["love".into()],
        },
        Emoji {
            emoji: "💚".into(),
            name: "green_heart".into(),
            keywords: vec!["love".into()],
        },
        Emoji {
            emoji: "💙".into(),
            name: "blue_heart".into(),
            keywords: vec!["love".into()],
        },
        Emoji {
            emoji: "💜".into(),
            name: "purple_heart".into(),
            keywords: vec!["love".into()],
        },
        Emoji {
            emoji: "🖤".into(),
            name: "black_heart".into(),
            keywords: vec!["love".into()],
        },
        Emoji {
            emoji: "🤍".into(),
            name: "white_heart".into(),
            keywords: vec!["love".into()],
        },
        Emoji {
            emoji: "🤎".into(),
            name: "brown_heart".into(),
            keywords: vec!["love".into()],
        },
        Emoji {
            emoji: "💔".into(),
            name: "broken_heart".into(),
            keywords: vec!["sad".into()],
        },
        Emoji {
            emoji: "❣️".into(),
            name: "heavy_heart_exclamation".into(),
            keywords: vec!["love".into()],
        },
        Emoji {
            emoji: "💕".into(),
            name: "two_hearts".into(),
            keywords: vec!["love".into()],
        },
        Emoji {
            emoji: "💞".into(),
            name: "revolving_hearts".into(),
            keywords: vec!["love".into()],
        },
        Emoji {
            emoji: "💓".into(),
            name: "heartbeat".into(),
            keywords: vec!["love".into()],
        },
        Emoji {
            emoji: "💗".into(),
            name: "heartpulse".into(),
            keywords: vec!["love".into()],
        },
        Emoji {
            emoji: "💖".into(),
            name: "sparkling_heart".into(),
            keywords: vec!["love".into()],
        },
        Emoji {
            emoji: "💘".into(),
            name: "cupid".into(),
            keywords: vec!["love".into(), "arrow".into()],
        },
        Emoji {
            emoji: "💝".into(),
            name: "gift_heart".into(),
            keywords: vec!["love".into(), "gift".into()],
        },
        Emoji {
            emoji: "💟".into(),
            name: "heart_decoration".into(),
            keywords: vec!["love".into()],
        },
        Emoji {
            emoji: "☮️".into(),
            name: "peace_symbol".into(),
            keywords: vec!["peace".into()],
        },
        Emoji {
            emoji: "✝️".into(),
            name: "latin_cross".into(),
            keywords: vec!["christian".into()],
        },
        Emoji {
            emoji: "☪️".into(),
            name: "star_and_crescent".into(),
            keywords: vec!["islam".into()],
        },
        Emoji {
            emoji: "🕉️".into(),
            name: "om".into(),
            keywords: vec!["hindu".into()],
        },
        Emoji {
            emoji: "✡️".into(),
            name: "star_of_david".into(),
            keywords: vec!["jewish".into()],
        },
        Emoji {
            emoji: "🔯".into(),
            name: "six_pointed_star".into(),
            keywords: vec![],
        },
        Emoji {
            emoji: "🕎".into(),
            name: "menorah".into(),
            keywords: vec!["jewish".into()],
        },
        Emoji {
            emoji: "☯️".into(),
            name: "yin_yang".into(),
            keywords: vec!["balance".into()],
        },
        Emoji {
            emoji: "☦️".into(),
            name: "orthodox_cross".into(),
            keywords: vec!["christian".into()],
        },
        Emoji {
            emoji: "🛐".into(),
            name: "place_of_worship".into(),
            keywords: vec!["religion".into()],
        },
        Emoji {
            emoji: "⛎".into(),
            name: "ophiuchus".into(),
            keywords: vec!["zodiac".into()],
        },
        Emoji {
            emoji: "♈".into(),
            name: "aries".into(),
            keywords: vec!["zodiac".into()],
        },
        Emoji {
            emoji: "♉".into(),
            name: "taurus".into(),
            keywords: vec!["zodiac".into()],
        },
        Emoji {
            emoji: "♊".into(),
            name: "gemini".into(),
            keywords: vec!["zodiac".into()],
        },
        Emoji {
            emoji: "♋".into(),
            name: "cancer".into(),
            keywords: vec!["zodiac".into()],
        },
        Emoji {
            emoji: "♌".into(),
            name: "leo".into(),
            keywords: vec!["zodiac".into()],
        },
        Emoji {
            emoji: "♍".into(),
            name: "virgo".into(),
            keywords: vec!["zodiac".into()],
        },
        Emoji {
            emoji: "♎".into(),
            name: "libra".into(),
            keywords: vec!["zodiac".into()],
        },
        Emoji {
            emoji: "♏".into(),
            name: "scorpius".into(),
            keywords: vec!["zodiac".into()],
        },
        Emoji {
            emoji: "♐".into(),
            name: "sagittarius".into(),
            keywords: vec!["zodiac".into()],
        },
        Emoji {
            emoji: "♑".into(),
            name: "capricorn".into(),
            keywords: vec!["zodiac".into()],
        },
        Emoji {
            emoji: "♒".into(),
            name: "aquarius".into(),
            keywords: vec!["zodiac".into()],
        },
        Emoji {
            emoji: "♓".into(),
            name: "pisces".into(),
            keywords: vec!["zodiac".into()],
        },
        Emoji {
            emoji: "🆔".into(),
            name: "id".into(),
            keywords: vec!["identity".into()],
        },
        Emoji {
            emoji: "⚛️".into(),
            name: "atom_symbol".into(),
            keywords: vec!["science".into()],
        },
        Emoji {
            emoji: "🔴".into(),
            name: "red_circle".into(),
            keywords: vec!["red".into()],
        },
        Emoji {
            emoji: "🟠".into(),
            name: "orange_circle".into(),
            keywords: vec!["orange".into()],
        },
        Emoji {
            emoji: "🟡".into(),
            name: "yellow_circle".into(),
            keywords: vec!["yellow".into()],
        },
        Emoji {
            emoji: "🟢".into(),
            name: "green_circle".into(),
            keywords: vec!["green".into()],
        },
        Emoji {
            emoji: "🔵".into(),
            name: "blue_circle".into(),
            keywords: vec!["blue".into()],
        },
        Emoji {
            emoji: "🟣".into(),
            name: "purple_circle".into(),
            keywords: vec!["purple".into()],
        },
        Emoji {
            emoji: "🟤".into(),
            name: "brown_circle".into(),
            keywords: vec!["brown".into()],
        },
        Emoji {
            emoji: "⚫".into(),
            name: "black_circle".into(),
            keywords: vec!["black".into()],
        },
        Emoji {
            emoji: "⚪".into(),
            name: "white_circle".into(),
            keywords: vec!["white".into()],
        },
        Emoji {
            emoji: "🔥".into(),
            name: "fire".into(),
            keywords: vec!["flame".into(), "hot".into(), "lit".into()],
        },
        Emoji {
            emoji: "✨".into(),
            name: "sparkles".into(),
            keywords: vec!["shiny".into(), "stars".into()],
        },
        Emoji {
            emoji: "⭐".into(),
            name: "star".into(),
            keywords: vec![],
        },
        Emoji {
            emoji: "🌟".into(),
            name: "star2".into(),
            keywords: vec!["glow".into()],
        },
        Emoji {
            emoji: "💫".into(),
            name: "dizzy".into(),
            keywords: vec!["star".into()],
        },
        Emoji {
            emoji: "✅".into(),
            name: "white_check_mark".into(),
            keywords: vec!["done".into(), "yes".into(), "tick".into()],
        },
        Emoji {
            emoji: "❌".into(),
            name: "x".into(),
            keywords: vec!["no".into(), "wrong".into()],
        },
        Emoji {
            emoji: "⚠️".into(),
            name: "warning".into(),
            keywords: vec!["alert".into(), "caution".into()],
        },
        Emoji {
            emoji: "🚀".into(),
            name: "rocket".into(),
            keywords: vec!["space".into(), "ship".into()],
        },
        Emoji {
            emoji: "💻".into(),
            name: "computer".into(),
            keywords: vec!["laptop".into(), "pc".into()],
        },
        Emoji {
            emoji: "📱".into(),
            name: "iphone".into(),
            keywords: vec!["phone".into(), "mobile".into()],
        },
        Emoji {
            emoji: "⌨️".into(),
            name: "keyboard".into(),
            keywords: vec!["type".into()],
        },
        Emoji {
            emoji: "🖱️".into(),
            name: "computer_mouse".into(),
            keywords: vec!["mouse".into()],
        },
        Emoji {
            emoji: "🖥️".into(),
            name: "desktop_computer".into(),
            keywords: vec!["computer".into(), "pc".into()],
        },
        Emoji {
            emoji: "💡".into(),
            name: "bulb".into(),
            keywords: vec!["idea".into(), "light".into()],
        },
        Emoji {
            emoji: "🔦".into(),
            name: "flashlight".into(),
            keywords: vec!["light".into(), "torch".into()],
        },
        Emoji {
            emoji: "📝".into(),
            name: "memo".into(),
            keywords: vec!["note".into(), "write".into()],
        },
        Emoji {
            emoji: "📖".into(),
            name: "book".into(),
            keywords: vec!["read".into()],
        },
        Emoji {
            emoji: "📚".into(),
            name: "books".into(),
            keywords: vec!["library".into()],
        },
        Emoji {
            emoji: "📁".into(),
            name: "file_folder".into(),
            keywords: vec!["folder".into(), "directory".into()],
        },
        Emoji {
            emoji: "📂".into(),
            name: "open_file_folder".into(),
            keywords: vec!["folder".into()],
        },
        Emoji {
            emoji: "🗂️".into(),
            name: "card_index_dividers".into(),
            keywords: vec!["organize".into()],
        },
        Emoji {
            emoji: "📅".into(),
            name: "date".into(),
            keywords: vec!["calendar".into()],
        },
        Emoji {
            emoji: "📆".into(),
            name: "calendar".into(),
            keywords: vec!["date".into(), "schedule".into()],
        },
        Emoji {
            emoji: "🗓️".into(),
            name: "spiral_calendar".into(),
            keywords: vec!["date".into()],
        },
        Emoji {
            emoji: "⏰".into(),
            name: "alarm_clock".into(),
            keywords: vec!["time".into(), "wake".into()],
        },
        Emoji {
            emoji: "⏱️".into(),
            name: "stopwatch".into(),
            keywords: vec!["time".into()],
        },
        Emoji {
            emoji: "⏲️".into(),
            name: "timer_clock".into(),
            keywords: vec!["time".into()],
        },
        Emoji {
            emoji: "🕐".into(),
            name: "clock1".into(),
            keywords: vec!["time".into()],
        },
    ]
}

pub fn search_emojis(query: &str) -> Vec<Emoji> {
    let query_lower = query.trim_start_matches(':').to_lowercase();
    let db = get_emoji_database();

    let mut results: Vec<(Emoji, i32)> = db
        .into_iter()
        .filter_map(|emoji| {
            // Exact name match
            if emoji.name == query_lower {
                return Some((emoji, 100));
            }

            // Name starts with query
            if emoji.name.starts_with(&query_lower) {
                return Some((emoji, 90));
            }

            // Name contains query
            if emoji.name.contains(&query_lower) {
                return Some((emoji, 70));
            }

            // Keyword match
            for keyword in &emoji.keywords {
                if keyword == &query_lower {
                    return Some((emoji, 80));
                }
                if keyword.starts_with(&query_lower) {
                    return Some((emoji, 60));
                }
                if keyword.contains(&query_lower) {
                    return Some((emoji, 50));
                }
            }

            None
        })
        .collect();

    // Sort by score descending
    results.sort_by(|a, b| b.1.cmp(&a.1));

    // Return top 20 results
    results.into_iter().take(20).map(|(e, _)| e).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test get_emoji_database
    #[test]
    fn test_emoji_database_not_empty() {
        let db = get_emoji_database();
        assert!(!db.is_empty());
    }

    #[test]
    fn test_emoji_database_contains_common_emojis() {
        let db = get_emoji_database();

        // Check for common emojis
        assert!(db.iter().any(|e| e.emoji == "😀"));
        assert!(db.iter().any(|e| e.emoji == "❤️"));
        assert!(db.iter().any(|e| e.emoji == "👍"));
        assert!(db.iter().any(|e| e.emoji == "🔥"));
    }

    #[test]
    fn test_emoji_struct_fields() {
        let db = get_emoji_database();
        let emoji = db.first().unwrap();

        // Each emoji should have non-empty name and emoji character
        assert!(!emoji.emoji.is_empty());
        assert!(!emoji.name.is_empty());
    }

    // Test search_emojis with various queries
    #[test]
    fn test_search_emojis_by_name() {
        let results = search_emojis("smile");
        assert!(!results.is_empty());

        // Should contain smile-related emojis
        assert!(results.iter().any(|e| e.name.contains("smile")));
    }

    #[test]
    fn test_search_emojis_by_keyword() {
        let results = search_emojis("happy");
        assert!(!results.is_empty());

        // Should find emojis with "happy" keyword
        assert!(results
            .iter()
            .any(|e| e.keywords.contains(&"happy".to_string())));
    }

    #[test]
    fn test_search_emojis_with_colon_prefix() {
        let results = search_emojis(":smile");
        assert!(!results.is_empty());

        // Colon prefix should be stripped and search should work
        assert!(results.iter().any(|e| e.name.contains("smile")));
    }

    #[test]
    fn test_search_emojis_exact_match() {
        let results = search_emojis("heart");
        assert!(!results.is_empty());

        // Should find heart emoji
        assert!(results.iter().any(|e| e.name == "heart"));
    }

    #[test]
    fn test_search_emojis_partial_match() {
        let results = search_emojis("grin");
        assert!(!results.is_empty());

        // Should find grinning emoji
        assert!(results.iter().any(|e| e.name.contains("grin")));
    }

    #[test]
    fn test_search_emojis_case_insensitive() {
        let results_lower = search_emojis("smile");
        let results_upper = search_emojis("SMILE");
        let results_mixed = search_emojis("SmILe");

        assert!(!results_lower.is_empty());
        assert!(!results_upper.is_empty());
        assert!(!results_mixed.is_empty());
    }

    #[test]
    fn test_search_emojis_no_results() {
        let results = search_emojis("xyznonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_emojis_returns_max_20() {
        let results = search_emojis("a"); // Very broad search
        assert!(results.len() <= 20);
    }

    #[test]
    fn test_search_emojis_fire() {
        let results = search_emojis("fire");
        assert!(!results.is_empty());
        assert!(results.iter().any(|e| e.emoji == "🔥"));
    }

    #[test]
    fn test_search_emojis_rocket() {
        let results = search_emojis("rocket");
        assert!(!results.is_empty());
        assert!(results.iter().any(|e| e.emoji == "🚀"));
    }

    #[test]
    fn test_search_emojis_love() {
        let results = search_emojis("love");
        assert!(!results.is_empty());
        // Should find multiple love-related emojis (hearts, etc.)
    }

    // Test Emoji struct serialization
    #[test]
    fn test_emoji_serialization() {
        let emoji = Emoji {
            emoji: "😀".to_string(),
            name: "grinning".to_string(),
            keywords: vec!["smile".to_string(), "happy".to_string()],
        };

        let json = serde_json::to_string(&emoji).unwrap();
        let deserialized: Emoji = serde_json::from_str(&json).unwrap();

        assert_eq!(emoji.emoji, deserialized.emoji);
        assert_eq!(emoji.name, deserialized.name);
        assert_eq!(emoji.keywords, deserialized.keywords);
    }

    #[test]
    fn test_emoji_clone() {
        let emoji = Emoji {
            emoji: "❤️".to_string(),
            name: "heart".to_string(),
            keywords: vec!["love".to_string()],
        };

        let cloned = emoji.clone();
        assert_eq!(emoji.emoji, cloned.emoji);
        assert_eq!(emoji.name, cloned.name);
        assert_eq!(emoji.keywords, cloned.keywords);
    }

    // Test search ranking
    #[test]
    fn test_search_emojis_exact_match_ranked_first() {
        let results = search_emojis("smile");

        if !results.is_empty() {
            // Exact match should be highly ranked
            let first = &results[0];
            assert!(
                first.name == "smile"
                    || first.name.starts_with("smile")
                    || first.name.contains("smile")
            );
        }
    }

    #[test]
    fn test_search_emojis_starts_with_ranked_high() {
        let results = search_emojis("thi");

        // Emojis that start with "thi" should be ranked higher
        if !results.is_empty() {
            // First result should start with or contain "thi"
            let first = &results[0];
            assert!(first.name.contains("thi") || first.keywords.iter().any(|k| k.contains("thi")));
        }
    }

    // Test specific emoji lookups
    #[test]
    fn test_search_emojis_thumbsup() {
        let results = search_emojis("thumbsup");
        assert!(!results.is_empty());
        assert!(results.iter().any(|e| e.emoji == "👍"));
    }

    #[test]
    fn test_search_emojis_check() {
        let results = search_emojis("check");
        assert!(!results.is_empty());
        assert!(results.iter().any(|e| e.emoji == "✅"));
    }

    #[test]
    fn test_search_emojis_warning() {
        let results = search_emojis("warning");
        assert!(!results.is_empty());
        assert!(results.iter().any(|e| e.emoji == "⚠️"));
    }
}
