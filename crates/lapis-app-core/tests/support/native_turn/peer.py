#!/usr/bin/python3
import sys, os, json, time

log = open(os.environ['LOG'], 'a', buffering=1)
log.write(json.dumps({'pid': os.getpid()}) + '\n')
scope = None
sequence = 0

def emit(kind, request, **body):
    global sequence
    sequence += 1
    frame = {'version': 3, 'type': kind, 'requestId': request,
             'sequence': sequence, **body}
    if scope:
        frame['sessionId'] = scope
    print(json.dumps(frame), flush=True)

print(json.dumps({'version': 3, 'type': 'ready'}), flush=True)
for line in sys.stdin:
    frame = json.loads(line)
    log.write(json.dumps(frame) + '\n')
    request = frame['requestId']
    kind = frame['type']
    if kind == 'warm':
        emit('warmed', request)
    elif kind == 'open':
        scope = frame['sessionId']
        emit('opened', request, adapterSessionId='provider-' + scope, session={})
    elif kind == 'configure':
        emit('configured', request, modeReady=frame.get('mode') != 'denied')
    elif kind == 'prompt':
        emit('started', request, adapterSessionId='provider-' + scope)
        time.sleep(0.4)
        chunks = ['x'] * 400 if frame['text'] in ['streaming-chunks', 'burst-chunks'] else ['reply café 📝']
        for text in chunks:
            emit('activity', request, update={'sessionUpdate': 'agent_message_chunk',
                 'content': {'type': 'text', 'text': text}})
            if frame['text'] == 'streaming-chunks':
                time.sleep(0.001)
        emit('record', request, record={'version': 1, 'taskId': request,
             'status': 'completed'}, sessionUsable=True, quiescent=True)
    elif kind == 'status':
        emit('status', request)
