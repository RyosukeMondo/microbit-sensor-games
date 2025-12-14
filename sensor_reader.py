# micro:bit Sensor Reader
# This program reads sensor values and outputs them to serial
# Format: timestamp,accel_x,accel_y,accel_z,temp,light_level

from microbit import *
import time

# Initial delay to allow serial connection
sleep(1000)

print("BBC micro:bit Sensor Reader v1.0")
print("timestamp,accel_x,accel_y,accel_z,temp,light")
print("---")

timestamp = 0

while True:
    # Read accelerometer (in milli-g, 1g = 1000)
    accel_x = accelerometer.get_x()
    accel_y = accelerometer.get_y()
    accel_z = accelerometer.get_z()

    # Read temperature (in Celsius)
    temp = temperature()

    # Read light level (0-255, using LED matrix as sensor)
    light = display.read_light_level()

    # Output as CSV
    print("{},{},{},{},{},{}".format(
        timestamp,
        accel_x,
        accel_y,
        accel_z,
        temp,
        light
    ))

    # Visual feedback - show heart beat
    if timestamp % 2 == 0:
        display.show(Image.HEART)
    else:
        display.show(Image.HEART_SMALL)

    timestamp += 1
    sleep(500)  # 500ms interval = 2Hz sampling rate
