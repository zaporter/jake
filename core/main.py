from pathlib import Path
from typing import Union
import uvicorn
from fastapi import FastAPI, Request, HTTPException

import fire
import transformers
import torch
from transformers import GenerationConfig, TextIteratorStreamer
from threading import Thread, Lock

from axolotl.utils.dict import DictDefault
from axolotl.common.cli import TrainerCliArgs, load_model_and_tokenizer
from axolotl.cli import do_inference, load_cfg, print_axolotl_text_art,load_datasets
from axolotl.common.cli import TrainerCliArgs
from axolotl.train import train

app = FastAPI()
model = None
tokenizer = None
streamer = None

STATUS_LOADING = "loading"
STATUS_GENERATING = "generating"
STATUS_DONE_GENERATING = "done_generating"
STATUS_READY = "ready"
STATUS_ERROR = "error"
statuslock = Lock()
# start statuslock protected
status = STATUS_READY
statusbody = {}
generated_text = ""
should_stop =  False
# end statuslock protected
generate_thread = None

@app.get("/")
def read_root():
    return {"Hello": "Jake"}

@app.post("/start")
def read_start():
    print("start")
    return {"hello": "world"}

@app.post("/status")
async def read_status():
    global status, statuslock, statusbody
    print("py:reading")
    with statuslock:
        statusclone = status
        bodyclone = statusbody
    print(f"py:read {statusclone} {bodyclone}")
    return {"status": statusclone, "body": bodyclone}

#
@app.post("/get_generated")
def read_get_generated():
    global status, statusbody, statuslock,generated_text
    with statuslock:
        if not status == STATUS_DONE_GENERATING:
            raise HTTPException(status_code=400, detail="Status was not Done Generating")
        status = STATUS_READY
        statusbody = {}
        generated_text = ""
        return {"text":generated_text}

@app.post("/infer")
async def read_infer(req : Request):
    print("infer")
    global status, statuslock,generate_thread
    with statuslock:
        if not (status == STATUS_READY or status == STATUS_DONE_GENERATING):
            raise HTTPException(status_code=400, detail="Status was not Ready")
    data = await req.json()
    prompt : str = data["prompt"]
    config : dict = data["config"]
    with statuslock:
        print("Generating started command.")
        status = STATUS_GENERATING

    generation_kwargs = dict(config=Path("./mistralif.yml"),infer_cfg=config, instruction=prompt)
    generate_thread = Thread(target=run, kwargs=generation_kwargs)
    generate_thread.start()
    return {}

@app.post("/stop")
def read_cancel():
    print("you got it brah. stopped")
    global should_stop, statuslock
    with statuslock:
        should_stop = True
    return {"Hello": "World"}


@app.post("/train")
def read_train():
    dotrain(Path("./mistralif.yml"))
    return {"Hello": "World"}

def infer(
    *,
    cfg: DictDefault,
    cli_args: TrainerCliArgs,
    infer_cfg: dict,
    instruction: str,
)->str:
    global model, tokenizer, streamer, statuslock, statusbody, generated_text, status, should_stop
    print("starting inference")
    if model is None or tokenizer is None:
        print("shits wack")
        raise Exception

    with statuslock:
        print("Generating started.")
        status = STATUS_GENERATING
        generated_text = ""
        should_stop = False
        statusbody = {"text":generated_text}
        streamer = TextIteratorStreamer(tokenizer)

    print("=" * 80)
    prompt = instruction.strip()
    batch = tokenizer(prompt, return_tensors="pt", add_special_tokens=True)

    print("=" * 40)
    model.eval()
    print("=" * 40)

    streamerthread = Thread(target=sync_text)
    streamerthread.start()
    with torch.no_grad():
        generation_config = GenerationConfig(
            repetition_penalty=infer_cfg["repetition_penalty"],
            max_new_tokens=infer_cfg["max_new_tokens"],
            temperature=infer_cfg["temperature"],
            top_p=infer_cfg["top_p"],
            top_k=infer_cfg["top_k"],
            do_sample=infer_cfg["do_sample"],
            use_cache=infer_cfg["use_cache"],
            return_dict_in_generate=infer_cfg["return_dict_in_generate"],
            output_attentions=infer_cfg["output_attentions"],
            output_hidden_states=infer_cfg["output_hidden_states"],
            output_scores=infer_cfg["output_scores"],
            bos_token_id=tokenizer.bos_token_id,
            eos_token_id=tokenizer.eos_token_id,
            pad_token_id=tokenizer.pad_token_id,
        )
        # streamer = TextStreamer(tokenizer)
        # generation_kwargs = dict(inputs=batch["input_ids"].to(cfg.device), streamer=streamer, max_new_tokens=200, generation_config=generation_config)
        # thread = Thread(target=model.generate, kwargs=generation_kwargs)

        # reset the streamer
        stopping_criteria = UserRequestedStopCriteria()
        generation = model.generate(inputs=batch["input_ids"].to(cfg.device), streamer=streamer, stopping_criteria=[stopping_criteria], generation_config=generation_config)

    streamerthread.join()

        # thread.start()

        # generated_text = ""

        # for new_text in streamer:
        #     generated_text += new_text
        #     print(new_text)
    print("Done infering.")
    with statuslock:
        status = STATUS_DONE_GENERATING
        statusbody = {"text": generated_text}
    # print("=" * 40)
    # print(generated_text)
    # result = tokenizer.decode(token_ids=generated["sequences"].cpu().tolist()[0],skip_special_tokens=True)
    # print(result)
    return generation

def sync_text():
    global statuslock, generated_text, streamer, statusbody
    if streamer is None:
        print("streamer is none")
        raise Exception
    for new_text in streamer:
        with statuslock:
            print(new_text)
            generated_text += new_text
            statusbody = {"text": generated_text}

def load(config: Path ):
    global model, tokenizer
    cfg = load_cfg(config)
    cfg.sample_packing = False
    parser = transformers.HfArgumentParser((TrainerCliArgs))
    cli_args, _ = parser.parse_args_into_dataclasses(
        return_remaining_strings=True
    )
    cli_args.inference = True
    model, tokenizer = load_model_and_tokenizer(cfg=cfg, cli_args=cli_args)
    default_tokens = {"unk_token": "<unk>", "bos_token": "<s>", "eos_token": "</s>"}

    print("1")
    for token, symbol in default_tokens.items():
        # If the token isn't already specified in the config, add it
        if not (cfg.special_tokens and token in cfg.special_tokens):
            tokenizer.add_special_tokens({token: symbol})


    print("2")
    if cfg.landmark_attention:
        from axolotl.monkeypatch.llama_landmark_attn import set_model_mem_id

        set_model_mem_id(model, tokenizer)
        model.set_mem_cache_args(
            max_seq_len=255, mem_freq=50, top_k=5, max_cache_size=None
        )

    print("3")
    model = model.to(cfg.device)

    print("finished model loading")

def run(config: Path, infer_cfg: dict, instruction: str)->str:
    parsed_cfg = load_cfg(config)
    parsed_cfg.sample_packing = False
    parser = transformers.HfArgumentParser((TrainerCliArgs))
    parsed_cli_args, _ = parser.parse_args_into_dataclasses(
        return_remaining_strings=True
    )
    parsed_cli_args.inference = True
    return infer(cfg=parsed_cfg, cli_args=parsed_cli_args, infer_cfg=infer_cfg, instruction=instruction)

class UserRequestedStopCriteria(transformers.StoppingCriteria):
    def __call__(self, input_ids: torch.LongTensor, scores: torch.FloatTensor, **kwargs) -> bool:
        global statuslock, should_stop
        with statuslock:
            return should_stop


if __name__ == "__main__":
    load(config=Path("./mistralif.yml"))
    uvicorn.run(app, host="0.0.0.0", port=9090)

def do_cli(config: Path = Path("examples/"), **kwargs):
    print("Hi!")
    # pylint: disable=duplicate-code
    print_axolotl_text_art()
    parsed_cfg = load_cfg(config, **kwargs)
    parser = transformers.HfArgumentParser((TrainerCliArgs))
    parsed_cli_args, _ = parser.parse_args_into_dataclasses(
        return_remaining_strings=True
    )
    parsed_cli_args.inference = True

    do_inference(cfg=parsed_cfg, cli_args=parsed_cli_args)


def dotrain(
    config: Path
        ):

    parsed_cfg = load_cfg(config)
    parser = transformers.HfArgumentParser((TrainerCliArgs))
    parsed_cli_args, _ = parser.parse_args_into_dataclasses(
        return_remaining_strings=True
    )

    parsed_cfg.local_rank = 1

    dataset_meta = load_datasets(cfg=parsed_cfg, cli_args=parsed_cli_args)
    train(cfg=parsed_cfg, cli_args=parsed_cli_args, dataset_meta=dataset_meta)

# if __name__ == "__main__":
#     fire.Fire(do_cli)
