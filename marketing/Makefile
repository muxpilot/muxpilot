# MuxPilot marketing media pipeline.
# Regenerate demo gifs/mp4s whenever the TUI changes.
#
#   make -C marketing regen     # render all deterministic demo tapes
#   make -C marketing demos     # alias for regen
#   make -C marketing real      # live OpenRouter coding-agent capture (needs key)
#   make -C marketing deps      # check vhs / ttyd / ffmpeg
#   make -C marketing clean     # remove rendered media + scratch

SHELL := /usr/bin/env bash
HERE  := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
MEDIA := $(HERE)../apps/web/public/media

.PHONY: regen demos real deps fonts clean help

help:
	@echo "MuxPilot marketing targets:"
	@echo "  regen   render all deterministic demo tapes -> apps/web/public/media"
	@echo "  demos   alias for regen"
	@echo "  real    live OpenRouter agent capture (needs OPENROUTER_API_KEY)"
	@echo "  deps    check vhs / ttyd / ffmpeg / recording font"
	@echo "  fonts   install JetBrainsMono Nerd Font Mono (the recording font)"
	@echo "  clean   remove rendered media + scratch"

# Install the exact monospace face the tapes record with.
fonts:
	@bash "$(HERE)scripts/install-fonts.sh"

# Deterministic demo media — the default, no network/agents required.
regen demos:
	@bash "$(HERE)regen.sh"

# Real coding-agent capture over OpenRouter. Gated on OPENROUTER_API_KEY.
real:
	@bash "$(HERE)scripts/real-agent.sh"

deps:
	@bash -c 'source "$(HERE)scripts/paths.sh"; source "$(HERE)scripts/compose.sh"; check_deps'

clean:
	@rm -rf "$(HERE).raw"
	@rm -f "$(MEDIA)"/picker.* "$(MEDIA)"/filter.* "$(MEDIA)"/navigate.* \
	       "$(MEDIA)"/help.* "$(MEDIA)"/real-agents.*
	@echo "cleaned rendered media + scratch"
