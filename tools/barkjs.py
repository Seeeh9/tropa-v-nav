"""Prints the page helpers for reading battle lines in AI Studio's speech playground.

    python tools/barkjs.py <who> [<who> ...]   # e.g. volhv volkolak

Paste the output into the page (the browser's JavaScript tool) after "Create new dialog" and
after picking the speaker's voice (docs/prompts/voice/barks.voices.txt). Then for each line:
`barkGo('<lang>', 'bark-<who>-<event>')`, ctrl+Return to run, download, and take the clips in with
`python tools/voice_in.py <lang> <keys>`. Only the lines of the named speakers are embedded.
"""
import json
import os
import re
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
VOICE = os.path.join(ROOT, "docs", "prompts", "voice")

HELPERS = r"""window.LANGNOTE={'ru-old':'The line is in Old Russian, read in the solemn old Church Slavonic manner: every vowel clear as written, E never read as YO; a capital letter inside a word marks its stressed vowel.','ru':'The line is in modern Russian.','en':'The line is in English, spoken with a faint Slavic weight.'};
window.voiceSet=(label,text)=>{ const t=[...document.querySelectorAll('textarea')].find(x=>(x.getAttribute('aria-label')||'')===label); if(!t) return 'NO '+label; const set=Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set; set.call(t,text); t.dispatchEvent(new Event('input',{bubbles:true})); t.dispatchEvent(new Event('change',{bubbles:true})); return label+' '+t.value.length; };
window.barkSay=(lang,key)=>{ const v=window.barkVoice[key.split('-')[1]]; document.querySelectorAll('audio').forEach(a=>a.pause()); window.voiceSet('Scene', v.persona+' '+window.LANGNOTE[lang]+' Speak only the line, once, with no extra words.'); return window.voiceSet('Speech block text', window.barkQ[lang][key])+' '+key; };
window.barkGo=(lang,key)=>{ const r=window.barkSay(lang,key); const t=[...document.querySelectorAll('textarea')].find(x=>(x.getAttribute('aria-label')||'')==='Speech block text'); t.focus(); t.setSelectionRange(t.value.length,t.value.length); return r; };
window.speakerNow=()=>[...document.querySelectorAll('button')].map(b=>b.textContent.trim()).filter(t=>t.startsWith('Speaker')).join('|');
window.voiceShow=(name)=>{ const els=[...document.querySelectorAll('*')].filter(e=>e.childElementCount===0 && (e.textContent||'').trim()===name); if(!els.length) return 'NO'; els[els.length-1].scrollIntoView({block:'center'}); return 'ok'; };
Object.keys(window.barkVoice).join(',')+' | '+window.speakerNow()"""


def main():
    who = set(sys.argv[1:])
    lines = {}
    for lang in ("ru-old", "ru", "en"):
        text = open(os.path.join(VOICE, f"barks.{lang}.txt"), encoding="utf-8").read()
        lines[lang] = {m.group(1): m.group(3) for m in re.finditer(r"^(bark-([a-z]+)-[a-z0-9]+) = (.*)$", text, re.M) if m.group(2) in who}
    voices = {}
    for line in open(os.path.join(VOICE, "barks.voices.txt"), encoding="utf-8"):
        m = re.match(r"^([a-z_]+) = (\w+) \| (.*)$", line.rstrip("\r\n"))
        if m and m.group(1) in who:
            voices[m.group(1)] = {"voice": m.group(2), "persona": m.group(3)}
    sys.stdout.reconfigure(encoding="utf-8")
    print(f"window.barkQ={json.dumps(lines, ensure_ascii=False)};window.barkVoice={json.dumps(voices, ensure_ascii=False)};")
    print(HELPERS)


if __name__ == "__main__":
    main()
