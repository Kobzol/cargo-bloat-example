import asyncio

import zmq
from zmq.asyncio import Context

context = Context()

router = context.socket(zmq.ROUTER)
router.setsockopt(zmq.ROUTER_MANDATORY, 1)
router.set_hwm(10)
router.bind("tcp://127.0.0.1:5555")


async def echo():
    while True:
        data = await router.recv_multipart()
        print("message: {}".format(data[1].decode()))
        await router.send_multipart(data)

loop = asyncio.get_event_loop()
loop.create_task(echo())
loop.run_forever()
