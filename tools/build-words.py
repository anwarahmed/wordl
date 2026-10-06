#!/usr/bin/env python3
"""Rebuild words/answers.txt and words/allowed.txt from SCOWL.

Usage: tools/build-words.py path/to/scowl-2020.12.07

SCOWL: http://wordlist.aspell.net/ (see words/SCOWL-COPYRIGHT).

  allowed.txt  every five-letter word up to SCOWL size 80, all spellings.
               These are the guesses the game accepts.
  answers.txt  the common ones (size <= 35, English/American spelling) with
               plurals, simple inflections and unsuitable words removed.
               Puzzle words are drawn from this list.

words/definitions.txt is not generated: it holds a hand-written meaning for each
answer, and this script reports where it and answers.txt have drifted apart. The
game's tests fail until they match again.
"""
import glob
import os
import re
import sys

# Words kept out of answers.txt. They stay valid guesses; they are just never the
# puzzle. The rules in inflected() catch most plurals and inflections; these are what
# reading the list by hand turned up, by kind.
BLOCKED = set("""
bitch boobs booby booty busty buxom dildo dykes enema fagot fecal feces harem horny
hussy kinky lynch nazis penis porno prick pubes pubic pussy rapes raped semen sluts
spank sperm spunk turds uteri vulva wench whore
""".split())  # sexual, vulgar

BLOCKED |= set("""
chink kraut negro nappy paddy queer sissy spick spics
fatty idiot leper loony moron
noose slave
""".split())  # slurs, insults aimed at a group or a disability, and what goes with them

BLOCKED |= set("""
bowel fetus mucus urine vomit
""".split())  # bodily and unpleasant as a surprise answer

BLOCKED |= set("""
cacti elves fungi geese genii oases pence radii teeth women
""".split())  # plurals the rules cannot see

BLOCKED |= set("""
cried dried fried plied pried shied spied tried
feted flied pends redid smote undid upped
""".split())  # verb forms: regular -ied, and odd or archaic ones

BLOCKED |= set("""
abler apter barer baser bluer coyer cuter direr freer gayer haler huger lamer laxer
muter nuder odder rawer rifer sager shyer sorer viler weest wryer
""".split())  # awkward comparatives

BLOCKED |= set("""
harry maria peter roman sally
""".split())  # read as names

BLOCKED |= set("""
dunno fiver gimme gonna gotta kinda lemme mamma multi psych sorta wanna
""".split())  # informal, or not a word on its own

BLOCKED |= set("""
dimer edger fiche halon infix liker newsy octal shire sizer tatty unman unsay unset
""".split())  # jargon, regional, or barely words: nothing worth learning from them

# Asked for by the user after the first read-through: these had been kept as
# "ordinary enough" and were to go as well.
BLOCKED |= set("""
finer fewer later least lower newer nicer older paler purer rarer riper ruder safer
saner surer tamer truer wider wiser worse worst
""".split())  # comparatives and superlatives, however everyday

BLOCKED |= set("""
arose awoke began begun blown borne broke built burnt chose clung crept dealt drank
drawn drove dwelt eaten flown flung froze given grown heard knelt known leapt meant
risen shone shook shown slain slept slung slunk spent stank stole stood stuck stung
stunk swept swore sworn swung taken threw woken woven wrote wrung
""".split())  # irregular past tenses and participles

BLOCKED |= set("""
bigot dunce ninny prude yokel
""".split())  # insults, even mild ones

BLOCKED |= set("""
bosom groin navel ovary
booze drunk lager vodka
""".split())  # intimate anatomy, and drink

BLOCKED |= set("""
duvet lorry tonne
""".split())  # British spellings and words (the list is American: color, humor)

# Hard vocabulary is welcome, and wanted: the oldest players are well-read
# thirteen-year-olds, the younger ones play with their help, and the user sees the game
# as a way for them to learn new words. A first pass blocked 220 "hard" words; nearly
# all came back. These few stayed out for being unpleasant, not for being difficult.
BLOCKED |= set("""
colic goner mange polyp privy retch swill tripe
""".split())  # unpleasant

# The game is played by children (the user said so). Beyond everything above, these
# are no answer for a child to be handed: harm and crime, adult themes, tobacco,
# drugs and gambling, put-downs and remarks about bodies, labels for kinds of people,
# and two that only get giggles. When in doubt, a word goes here.
BLOCKED |= set("""
abort abuse arson choke drown fatal felon gouge rifle
bawdy elope erect flirt grope lover lurid lusty naked nymph thong
cigar opium poker rummy snuff tipsy wager
cocky dummy dumpy freak loser obese pudgy tramp
caste hippy macho pagan seedy taboo
junta penal
""".split())  # not for children


def load(root, max_size, cats=None, length=None):
    words = set()
    for path in glob.glob(os.path.join(root, "final", "*-words.*")):
        cat, size = os.path.basename(path).rsplit("-words.", 1)
        if int(size) > max_size or (cats and cat not in cats):
            continue
        with open(path, encoding="latin1") as fh:
            for word in fh:
                word = word.strip()
                if re.fullmatch("[a-z]+", word) and length in (None, len(word)):
                    words.add(word)
    return words


def inflected(word, stems):
    if word.endswith("s") and not word.endswith("ss") and word[:-1] in stems:
        return True
    if word.endswith("ies") or (word.endswith("es") and word[:-2] in stems):
        return True
    if word.endswith("ed") and (word[:-2] in stems or word[:-1] in stems):
        return True
    if word.endswith("ier") and word[:-3] + "y" in stems:
        return True
    if word.endswith("ing") and (
        word[:-3] in stems or word[:-3] + "e" in stems or word[:-4] + "ie" in stems
    ):
        return True
    return False


def main():
    root = sys.argv[1]
    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "words")
    stems = load(root, 60)
    allowed = load(root, 80, length=5)
    common = load(root, 35, {"english", "american"}, 5)
    answers = {w for w in common if w not in BLOCKED and not inflected(w, stems)}
    allowed |= answers
    for name, words in (("answers.txt", answers), ("allowed.txt", allowed)):
        with open(os.path.join(out, name), "w") as fh:
            fh.write("\n".join(sorted(words)) + "\n")
        print(f"{name}: {len(words)} words")

    # definitions.txt is written by hand, one line per answer. Say what is out of step.
    with open(os.path.join(out, "definitions.txt")) as fh:
        defined = {line.split("\t", 1)[0] for line in fh if line.strip()}
    for label, words in (("need a definition", answers - defined), ("have a definition but are no longer answers", defined - answers)):
        if words:
            print(f"definitions.txt: {len(words)} words {label}: {' '.join(sorted(words))}")


if __name__ == "__main__":
    main()
