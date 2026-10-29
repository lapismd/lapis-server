#!/usr/bin/python3
import sys, os, json, time

log = open(os.environ['LOG'], 'a', buffering=1)
log.write(json.dumps({'pid': os.getpid()}) + '\n')
scope, variant, sequence = None, 'ordinary', 0

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
    request, kind = frame['requestId'], frame['type']
    if kind == 'warm':
        emit('warmed', request)
    elif kind == 'open':
        scope = frame['sessionId']
        emit('opened', request, adapterSessionId='provider-' + scope, session={})
    elif kind == 'configure':
        variant = frame.get('model') or 'ordinary'
        if frame.get('mode') == 'denied':
            while not os.path.exists(os.environ['LOG'] + '.configure.release'):
                time.sleep(0.01)
        emit('configured', request, modeReady=frame.get('mode') != 'denied')
    elif kind == 'prompt':
        emit('started', request, adapterSessionId='provider-' + scope)
        if variant.startswith('held-'):
            release = os.environ['LOG'] + '.release'
            while not os.path.exists(release) and not os.path.exists(release + '.' + variant):
                time.sleep(0.01)
        for text in [variant + ':α', ':β', ':終']:
            emit('activity', request, update={'sessionUpdate': 'agent_message_chunk',
                 'content': {'type': 'text', 'text': text}})
            time.sleep(0.025)
        emit('record', request, record={'version': 1, 'taskId': request,
             'status': 'completed'}, sessionUsable=True, quiescent='unknown' not in variant)
    elif kind == 'status':
        emit('status', request)
