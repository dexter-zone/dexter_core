from model import DexterModel
import asyncio


async def execute_simulation():
    dexter_simulation = DexterModel()
    i = 0

    await dexter_simulation.update_agents_state()

    while i < 10000: 
        await dexter_simulation.step()
        i = i + 1




if __name__ == "__main__":
    loop = asyncio.get_event_loop()

    while(1):
        try:
            loop.run_until_complete(execute_simulation())
        except Exception as e:
            print(e)  
            if e == KeyboardInterrupt:
                break
            # pass

    # asyncio.sleep(59*59)
