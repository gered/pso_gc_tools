cmake_minimum_required(VERSION 3.16)
project(pso_gc_tools C)

set(CMAKE_C_STANDARD 99)

include_directories(/usr/local/include)

#find_package(Iconv REQUIRED)

find_library(SYLVERANT_LIBRARY sylverant REQUIRED)

# decrypt_packets
add_executable(decrypt_packets decrypt_packets.c utils.c)
target_link_libraries(decrypt_packets ${SYLVERANT_LIBRARY})

# gen_qst_header
add_executable(gen_qst_header gen_qst_header.c quests.c utils.c)
target_link_libraries(gen_qst_header ${SYLVERANT_LIBRARY})
#add_executable(gen_qst_header gen_qst_header.c textconv.c quests.c utils.c)
#target_link_libraries(gen_qst_header ${SYLVERANT_LIBRARY} ${ICONV_LIBRARIES})
#target_compile_definitions(gen_qst_header PRIVATE ICONV_CONST=${ICONV_CONST})
#target_include_directories(gen_qst_header PRIVATE ${ICONV_INCLUDE_DIR})

# bindat_to_gcdl
add_executable(bindat_to_gcdl bindat_to_gcdl.c quests.c fuzziqer_prs.c utils.c)
target_link_libraries(bindat_to_gcdl ${SYLVERANT_LIBRARY})

# gci_extract
add_executable(gci_extract gci_extract.c quests.c fuzziqer_prs.c utils.c)
target_link_libraries(gci_extract ${SYLVERANT_LIBRARY})

# quest_info
add_executable(quest_info quest_info.c quests.c fuzziqer_prs.c utils.c)
target_link_libraries(quest_info ${SYLVERANT_LIBRARY})
