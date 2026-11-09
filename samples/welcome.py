#!/usr/bin/env python3
"""Welcome to KateCraft.

This sample file is here so the editor opens with something to look at.
Type anywhere, then use File > Save As to keep your own copy.
"""

import os
import sys
from dataclasses import dataclass


@dataclass
class Session:
    name: str
    tabs: list = None

    def summary(self) -> str:
        count = len(self.tabs or [])
        return f"{self.name}: {count} open tabs"


def main(argv):
    session = Session("welcome", ["welcome.py", "notes.txt"])
    for index, tab in enumerate(session.tabs):
        print(index, tab)

    # Shortcuts worth knowing:
    #   Ctrl+F  find        Ctrl+R  replace
    #   Ctrl+S  save        Ctrl+G  go to a line
    width = max(len(tab) for tab in session.tabs)
    print("widest tab name:", width)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
