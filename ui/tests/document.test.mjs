import {test} from 'node:test';
import assert from 'node:assert/strict';
import {meetingGroup, meetingDate, noteSections, taskLine, timeLabel, durationLabel} from '../lib/document.ts';
test('calendar groups use local midnight and Monday week boundaries',()=>{
 const now=new Date(2026,9,21,12);
 assert.equal(meetingGroup(new Date(2026,9,21).getTime(),now),'Today');
 assert.equal(meetingGroup(new Date(2026,9,19).getTime(),now),'This week');
 assert.equal(meetingGroup(new Date(2026,9,18).getTime(),now),'Earlier');
 assert.match(meetingDate(new Date(2026,9,20,13,11).getTime(),now),/^Yesterday · 13:11$/);
 const weekday=new Intl.DateTimeFormat(undefined,{weekday:'short'}).format(new Date(2026,9,15));
 assert.equal(meetingDate(new Date(2026,9,15,9,30).getTime(),now),`${weekday} · 09:30`);
 assert.doesNotMatch(meetingDate(new Date(2026,9,14,9,30).getTime(),now),new RegExp(`^${weekday}`));
 assert.equal(timeLabel(87000),'01:27');
 assert.deepEqual([22900,87000,599999,2580000].map(durationLabel),['22 s','1 min 27 s','9 min 59 s','43 min']);
});
test('task positions preserve original Markdown line numbers and owners are explicit',()=>{
 const sections=noteSections('### Action items\n- [ ] You: Send it [01:20]\n\n### Overview\n- Text\n<details><summary>Transcript</summary>\n### Not a section');
 assert.equal(sections.length,2);
 assert.equal(sections[0].lines[0].line,1);
 assert.deepEqual(taskLine(sections[0].lines[0].text),{done:false,owner:'You',text:'Send it [01:20]'});
 assert.equal(taskLine('- None'),null);
 assert.equal(taskLine('- [x] Send it').owner,'');
 assert.equal(noteSections('### Topic\n```\n### Code\n```').length,1);
});
