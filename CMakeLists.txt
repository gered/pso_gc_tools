cmake_minimum_required(VERSION 3.16)
project(psogc_quest_tools C)

set(CMAKE_C_STANDARD 99)

include_directories(/usr/local/include)

find_library(SYLVERANT_LIBRARY sylverant REQUIRED)

# decrypt_packets
add_executable(decrypt_packets decrypt_packets/main.c)
target_link_libraries(decrypt_packets ${SYLVERANT_LIBRARY})

