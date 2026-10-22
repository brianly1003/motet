import {test} from 'node:test';
import assert from 'node:assert/strict';
import {renderNotes, displayTranscript} from '../lib/md.ts';
test('notes cannot execute HTML or load remote resources',()=>{
 const html=renderNotes('<script>alert(1)</script>\n\n![secret](https://example.com/pixel)\n\n[Open](https://example.com)');
 assert.ok(!html.includes('<script>'));
 assert.ok(!html.includes('<img'));
 assert.ok(!html.includes('href='));
 assert.ok(!html.includes('https://example.com'));
});
test('notes retain structure and separate the transcript',()=>{
 const html=renderNotes('### Action items\n\n- Alex: ship Friday\n\n<details><summary>Transcript</summary>\nprivate transcript');
 assert.ok(html.includes('<h3>Action items</h3>'));
 assert.ok(html.includes('Alex: ship Friday'));
 assert.ok(!html.includes('private transcript'));
});
test('transcript shows You for the mic and leaves other speakers alone',()=>{
 const out=displayTranscript('[00:01] Them: Me: is a word\n[00:03] Me: Sure\n[00:05] Speaker 2: ok');
 assert.equal(out,'[00:01] Them: Me: is a word\n[00:03] You: Sure\n[00:05] Speaker 2: ok');
});

test('every time within the meeting becomes a chip; others stay text; untrusted text is escaped', () => {
  const html=renderNotes('- [01:20] and [00:05] <script>alert(1)</script> [99:59] [01:75]',87000);
  assert.match(html,/class="src" data-ms="80000"/);
  assert.match(html,/class="src" data-ms="5000"/);
  assert.ok(html.includes('[99:59]') && html.includes('[01:75]'));
  assert.ok(!html.includes('<script>'));
  assert.ok(!renderNotes('[01:20]').includes('class="src"'), 'no audio, no chips');
});
test('editable blocks carry their notes.md line range', () => {
  const html=renderNotes('- one\n- two\n\nPara line\nnext line',0,10);
  assert.match(html,/<li data-lines="10,11">one/);
  assert.match(html,/<li data-lines="11,13">two/); // a trailing blank line belongs to the item; saving keeps it
  assert.match(html,/<p data-lines="13,15">Para line/);
  assert.ok(!renderNotes('- one').includes('data-lines'));
});
test('a leading time chip moves to the end of the line', () => {
  const html=renderNotes('- [02:05] The **dashboard** is done.',300000);
  assert.match(html,/done\. <button type="button" class="src" data-ms="125000"/);
  assert.ok(!html.includes('<li><button'));
  assert.match(renderNotes('[02:05]',300000),/<p><button/,'a lone time stays put');
});
