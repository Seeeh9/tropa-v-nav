import ctypes
from ctypes import wintypes
import time

user32 = ctypes.windll.user32

INPUT_KEYBOARD = 1
KEYEVENTF_KEYUP = 0x0002
KEYEVENTF_UNICODE = 0x0004
VK_RETURN = 0x0D


class MOUSEINPUT(ctypes.Structure):
  _fields_ = [
      ("dx", wintypes.LONG),
      ("dy", wintypes.LONG),
      ("mouseData", wintypes.DWORD),
      ("dwFlags", wintypes.DWORD),
      ("time", wintypes.DWORD),
      ("dwExtraInfo", ctypes.c_size_t),
  ]


class KEYBDINPUT(ctypes.Structure):
  _fields_ = [
      ("wVk", wintypes.WORD),
      ("wScan", wintypes.WORD),
      ("dwFlags", wintypes.DWORD),
      ("time", wintypes.DWORD),
      ("dwExtraInfo", ctypes.c_size_t),
  ]


class HARDWAREINPUT(ctypes.Structure):
  _fields_ = [
      ("uMsg", wintypes.DWORD),
      ("wParamL", wintypes.WORD),
      ("wParamH", wintypes.WORD),
  ]


class INPUT_UNION(ctypes.Union):
  _fields_ = [
      ("mi", MOUSEINPUT),
      ("ki", KEYBDINPUT),
      ("hi", HARDWAREINPUT),
  ]


class INPUT(ctypes.Structure):
  _fields_ = [("type", wintypes.DWORD), ("union", INPUT_UNION)]


def send_char_unicode(ch):
  code = ord(ch)
  inp_down = INPUT()
  inp_down.type = INPUT_KEYBOARD
  inp_down.union.ki.wVk = 0
  inp_down.union.ki.wScan = code
  inp_down.union.ki.dwFlags = KEYEVENTF_UNICODE

  inp_up = INPUT()
  inp_up.type = INPUT_KEYBOARD
  inp_up.union.ki.wVk = 0
  inp_up.union.ki.wScan = code
  inp_up.union.ki.dwFlags = KEYEVENTF_UNICODE | KEYEVENTF_KEYUP

  user32.SendInput(1, ctypes.byref(inp_down), ctypes.sizeof(INPUT))
  time.sleep(0.03)
  user32.SendInput(1, ctypes.byref(inp_up), ctypes.sizeof(INPUT))
  time.sleep(0.06)


def send_enter():
  inp_down = INPUT()
  inp_down.type = INPUT_KEYBOARD
  inp_down.union.ki.wVk = VK_RETURN
  inp_down.union.ki.wScan = 0
  inp_down.union.ki.dwFlags = 0

  inp_up = INPUT()
  inp_up.type = INPUT_KEYBOARD
  inp_up.union.ki.wVk = VK_RETURN
  inp_up.union.ki.wScan = 0
  inp_up.union.ki.dwFlags = KEYEVENTF_KEYUP

  user32.SendInput(1, ctypes.byref(inp_down), ctypes.sizeof(INPUT))
  time.sleep(0.05)
  user32.SendInput(1, ctypes.byref(inp_up), ctypes.sizeof(INPUT))


WAIT_SECONDS = 7200

print(f"Скрипт запущен. Ждем 2 часа...")
print("Поставьте фокус в нужное поле ввода.")

time.sleep(WAIT_SECONDS)

for char in "продолжай":
  send_char_unicode(char)

time.sleep(0.3)
send_enter()

print("Готово! Набрано и отправлено.")
